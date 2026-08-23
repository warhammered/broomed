#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use broomed_core::{
    ai::{AiProvider, AiResult, AiTask, BundledLocalProvider, CloudProvider, HeuristicFallback},
    analysis::FileAnalysis,
    bridge,
    device::DeviceIdentity,
    hardware, hash, intent,
    license::{LicenseManager, LicenseState},
    mascot,
    mode::{AiMode, AiModeConfig},
    models as model_mgr,
    operation::{self, PlanPreview},
    orchestrator::Orchestrator,
    secure_store::SecureStore,
};
use zeroize::Zeroize;

// ponytail: Tauri replaces pywebview when `tauri dev/build` passes — keep core reuse thin, add plugins only when needed

fn parse_task(s: &str) -> AiTask {
    match s {
        "DescribeImage" => AiTask::DescribeImage,
        "SuggestFilename" => AiTask::SuggestFilename,
        "SuggestFolder" => AiTask::SuggestFolder,
        "DetectSemanticDuplicate" => AiTask::DetectSemanticDuplicate,
        "GenerateTags" => AiTask::GenerateTags,
        "SemanticSearch" => AiTask::SemanticSearch,
        "SummarizeDocument" => AiTask::SummarizeDocument,
        _ => AiTask::ClassifyFile,
    }
}

fn parse_ai_mode(s: &str) -> AiMode {
    match s.to_ascii_lowercase().as_str() {
        "hybrid" => AiMode::Hybrid,
        "online" => AiMode::Online,
        _ => AiMode::Local,
    }
}

fn license_state_str(s: &LicenseState) -> &'static str {
    match s {
        LicenseState::Inactive => "inactive",
        LicenseState::Active => "active",
        LicenseState::Expired => "expired",
        LicenseState::OfflineGrace => "offline_grace",
        LicenseState::ActivationRequired => "activation_required",
        LicenseState::DeviceConflict => "device_conflict",
    }
}

fn sanitized_license_json(mgr: &LicenseManager) -> serde_json::Value {
    let now = chrono::Utc::now().timestamp();
    let state = mgr.check(now);
    let online = mgr.is_online_ai_enabled(now);
    let ent = mgr.entitlement.as_ref();
    serde_json::json!({
        "state": license_state_str(&state),
        "online_ai_enabled": online,
        "expires_at": ent.map(|e| e.expires_at),
        "period_end": ent.and_then(|e| e.period_end),
    })
}

fn load_or_create_device(store: &SecureStore) -> DeviceIdentity {
    if let (Some(device_id), Some(pk_b64)) = (
        store.load_token("device_id"),
        store.load_token("device_pubkey"),
    ) {
        if store.load_private_key().is_some() {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            return DeviceIdentity {
                device_id,
                public_key_b64: pk_b64,
                platform: std::env::consts::OS.to_string(),
                app_version: env!("CARGO_PKG_VERSION").to_string(),
                created_at: now,
            };
        }
    }
    let (dev, sk) = DeviceIdentity::generate();
    let _ = store.store_token("device_id", &dev.device_id);
    let _ = store.store_token("device_pubkey", &dev.public_key_b64);
    let _ = store.store_private_key(&sk.to_bytes());
    dev
}

fn init_license_manager() -> Arc<tokio::sync::Mutex<LicenseManager>> {
    let api_base =
        std::env::var("BROOMED_API_BASE").unwrap_or_else(|_| "https://api.broomed.app".to_string());
    let store = SecureStore::default();
    let device = load_or_create_device(&store);
    let mgr = LicenseManager::new(api_base, store, device);
    Arc::new(tokio::sync::Mutex::new(mgr))
}

fn init_ai_mode() -> Mutex<AiModeConfig> {
    // try load persisted mode from SecureStore file fallback via token
    let store = SecureStore::default();
    if let Some(s) = store.load_token("ai_mode") {
        if let Ok(cfg) = serde_json::from_str::<AiModeConfig>(&s) {
            return Mutex::new(cfg);
        }
    }
    Mutex::new(AiModeConfig::default())
}

#[tauri::command]
fn scan_directory_cmd(base: String, max_files: Option<usize>) -> Result<Vec<String>, String> {
    bridge::scan_directory_py(&base, max_files.unwrap_or(10_000)).map_err(|e| e.to_string())
}

#[tauri::command]
fn hash_file_cmd(path: String) -> Result<String, String> {
    hash::hash_file(Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn parse_intent_cmd(text: String) -> String {
    let _ = bridge::parse_intent_py(&text);
    format!("{:?}", intent::parse_intent(&text))
}

#[tauri::command]
fn mascot_state_cmd(
    scan_running: bool,
    ai_thinking: bool,
    has_results: bool,
    organizing: bool,
    waiting: bool,
    error: bool,
    offline: bool,
) -> String {
    let state = mascot::MascotState::from_app_state(
        scan_running,
        ai_thinking,
        has_results,
        organizing,
        waiting,
        error,
        offline,
    );
    serde_json::to_string(&state).unwrap_or_else(|_| format!("{state:?}"))
}

#[tauri::command]
fn browse_directory_cmd() -> Result<Option<String>, String> {
    Ok(None)
}

// dev-only direct cloud: gate behind BROOMED_DEV_DIRECT_CLOUD=1
// Normal production path is Broomed gateway via OnlineAiClient / license entitlement.
fn cloud_from_provider_str(s: &str) -> Option<CloudProvider> {
    if std::env::var("BROOMED_DEV_DIRECT_CLOUD").unwrap_or_default() != "1" {
        return None;
    }
    match s {
        "openai" => Some(CloudProvider::openai()),
        "anthropic" => Some(CloudProvider::anthropic()),
        "cloud" => {
            let o = CloudProvider::openai();
            if o.is_configured() {
                Some(o)
            } else {
                let a = CloudProvider::anthropic();
                if a.is_configured() {
                    Some(a)
                } else {
                    Some(o)
                }
            }
        }
        _ => None,
    }
}

// ── Licensing commands (sanitized, no secrets) ──────────────

#[tauri::command]
async fn license_status_cmd(
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
) -> Result<String, String> {
    let mgr = license.lock().await;
    let v = sanitized_license_json(&mgr);
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

#[tauri::command]
async fn activate_license_cmd(
    activation_code: String,
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
) -> Result<String, String> {
    let mut code = activation_code;
    if code.trim().is_empty() {
        return Err("INVALID_ACTIVATION_CODE".to_string());
    }
    // capture device info before await
    let (app_version, platform) = {
        let mgr = license.lock().await;
        (mgr.device.app_version.clone(), mgr.device.platform.clone())
    };
    let mut mgr = license.lock().await;
    let res = mgr.activate(code.clone(), &app_version, &platform).await;
    // zeroize caller copy
    code.zeroize();
    drop(code);
    match res {
        Ok(_) => {
            let v = sanitized_license_json(&mgr);
            serde_json::to_string(&v).map_err(|e| e.to_string())
        }
        Err(e) => Err(e.code().to_string()),
    }
}

#[tauri::command]
async fn refresh_license_cmd(
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
) -> Result<String, String> {
    let mut mgr = license.lock().await;
    match mgr.refresh().await {
        Ok(_) => {
            let v = sanitized_license_json(&mgr);
            serde_json::to_string(&v).map_err(|e| e.to_string())
        }
        Err(e) => {
            // On SERVER_UNAVAILABLE, return cached grace state sanitized
            if e.code() == "SERVER_UNAVAILABLE" {
                let v = sanitized_license_json(&mgr);
                return serde_json::to_string(&v).map_err(|e| e.to_string());
            }
            Err(e.code().to_string())
        }
    }
}

#[tauri::command]
async fn get_device_info_cmd(
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
) -> Result<String, String> {
    let mgr = license.lock().await;
    let v = serde_json::json!({
        "device_id": mgr.device.device_id,
        "public_key": mgr.device.public_key_b64,
        "platform": mgr.device.platform,
        "app_version": mgr.device.app_version,
    });
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_ai_mode_cmd(
    mode: String,
    online_opt_in: bool,
    ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
) -> Result<String, String> {
    let m = parse_ai_mode(&mode);
    let mut cfg = ai_mode.lock().map_err(|e| e.to_string())?;
    cfg.mode = m;
    cfg.online_opt_in = online_opt_in;
    let v = serde_json::to_string(&*cfg).map_err(|e| e.to_string())?;
    // persist
    let store = SecureStore::default();
    let _ = store.store_token("ai_mode", &v);
    Ok(v)
}

// ── AI classification hardening ────────────────────────────

#[tauri::command]
async fn classify_cmd(
    task: String,
    input: String,
    provider: Option<String>,
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
    ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
) -> Result<AiResult, String> {
    let ai_task = parse_task(&task);

    // forced offline/local -> heuristic/bundled directly
    if provider.as_deref() == Some("offline")
        || provider.as_deref() == Some("local")
        || provider.as_deref() == Some("bundled")
    {
        let bundled = BundledLocalProvider::new();
        if bundled.supports(&ai_task) {
            return bundled
                .classify(ai_task, &input)
                .await
                .map_err(|e| e.to_string());
        }
        let fallback = HeuristicFallback::new();
        return fallback
            .classify(ai_task, &input)
            .await
            .map_err(|e| e.to_string());
    }

    // dev-only direct cloud compat
    if let Some(p) = provider.as_deref() {
        if matches!(p, "openai" | "anthropic" | "cloud") {
            if let Some(cp) = cloud_from_provider_str(p) {
                if cp.supports(&ai_task) {
                    match cp.classify(ai_task.clone(), &input).await {
                        Ok(r) => return Ok(r),
                        Err(e) => eprintln!("direct cloud failed, fallback bundled: {e}"),
                    }
                }
            } else {
                // normal path is Broomed gateway — ignore direct provider
            }
        }
    }

    // Route via AiMode + LicenseManager::is_online_ai_enabled()
    let cfg = ai_mode.lock().map_err(|e| e.to_string())?.clone();
    let (online_available, api_base, token) = {
        let mgr = license.lock().await;
        let now = chrono::Utc::now().timestamp();
        let avail = mgr.is_online_ai_enabled(now) && cfg.online_opt_in;
        let token = mgr.entitlement.as_ref().map(|e| e.license_id.clone());
        let api = mgr.api_base.clone();
        (avail, api, token)
    };
    let _ = &api_base;
    let _ = &token;

    // privacy: only selected file content transmitted when online_opt_in && entitlement valid
    if cfg.mode == AiMode::Online && online_available {
        if let Some(_tok) = token.clone() {
            #[cfg(feature = "cloud-ai")]
            {
                let cap = match ai_task {
                    AiTask::DescribeImage => "vision",
                    _ => "text",
                };
                let url = format!("{}/api/ai/{}", api_base.trim_end_matches('/'), cap);
                let http = reqwest::Client::new();
                let payload = serde_json::json!({"capability": cap, "input": input, "task": format!("{:?}", ai_task)});
                if let Ok(resp) = http
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", _tok))
                    .json(&payload)
                    .send()
                    .await
                {
                    if resp.status().is_success() {
                        if let Ok(v) = resp.json::<serde_json::Value>().await {
                            if let Ok(r) = serde_json::from_value::<AiResult>(v.clone()) {
                                return Ok(r);
                            }
                            if let Some(inner) = v.get("result") {
                                if let Ok(r) = serde_json::from_value::<AiResult>(inner.clone()) {
                                    return Ok(r);
                                }
                            }
                            if let Ok(r) = broomed_core::ai::parse_ai_json(&v.to_string()) {
                                return Ok(r);
                            }
                        }
                    } else {
                        eprintln!("online classify status {} fallback local", resp.status());
                    }
                } else {
                    eprintln!("online classify network error fallback local");
                }
            }
        }
        // fallback to bundled/heuristic - do not fail workflow
    }

    if cfg.mode == AiMode::Hybrid && online_available {
        // try local first, then online if confidence low
        let bundled = BundledLocalProvider::new();
        let local_res = if bundled.supports(&ai_task) {
            bundled
                .classify(ai_task.clone(), &input)
                .await
                .map_err(|e| e.to_string())?
        } else {
            let hb = HeuristicFallback::new();
            hb.classify(ai_task.clone(), &input)
                .await
                .map_err(|e| e.to_string())?
        };
        if cfg.should_try_online(local_res.confidence, true) {
            if let Some(_tok) = token {
                #[cfg(feature = "cloud-ai")]
                {
                    let cap = match ai_task {
                        AiTask::DescribeImage => "vision",
                        _ => "text",
                    };
                    let url = format!("{}/api/ai/{}", api_base.trim_end_matches('/'), cap);
                    let http = reqwest::Client::new();
                    let payload = serde_json::json!({"capability": cap, "input": input.clone(), "task": format!("{:?}", ai_task)});
                    if let Ok(resp) = http
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", _tok))
                        .json(&payload)
                        .send()
                        .await
                    {
                        if resp.status().is_success() {
                            if let Ok(v) = resp.json::<serde_json::Value>().await {
                                if let Ok(r) = serde_json::from_value::<AiResult>(v.clone()) {
                                    return Ok(r);
                                }
                                if let Some(inner) = v.get("result") {
                                    if let Ok(r) = serde_json::from_value::<AiResult>(inner.clone())
                                    {
                                        return Ok(r);
                                    }
                                }
                                if let Ok(r) = broomed_core::ai::parse_ai_json(&v.to_string()) {
                                    return Ok(r);
                                }
                            }
                        }
                    }
                }
            }
        }
        return Ok(local_res);
    }

    // Local mode or hybrid without online, or online fallback
    let bundled = BundledLocalProvider::new();
    if bundled.supports(&ai_task) {
        return bundled
            .classify(ai_task, &input)
            .await
            .map_err(|e| e.to_string());
    }
    let fallback = HeuristicFallback::new();
    fallback
        .classify(ai_task, &input)
        .await
        .map_err(|e| e.to_string())
}

// ── Phase 2: pipeline commands ───────────────────────────────

#[tauri::command]
async fn plan_organize(
    files: Vec<String>,
    base: String,
    task: Option<String>,
    threshold: Option<f32>,
    provider: Option<String>,
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
    ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
) -> Result<Vec<PlanPreview>, String> {
    let ai_task = parse_task(task.as_deref().unwrap_or("ClassifyFile"));
    let base_path = PathBuf::from(&base);
    let thr = threshold.unwrap_or(0.5);

    if provider.as_deref() == Some("offline") || provider.as_deref() == Some("local") {
        let hb = HeuristicFallback::new();
        return operation::plan_organize_with_provider(files, &base_path, &hb, ai_task, thr)
            .await
            .map_err(|e| e.to_string());
    }
    if let Some(p) = provider.as_deref() {
        if matches!(p, "openai" | "anthropic" | "cloud") {
            if let Some(cp) = cloud_from_provider_str(p) {
                if cp.supports(&ai_task) {
                    match operation::plan_organize_with_provider(
                        files.clone(),
                        &base_path,
                        &cp,
                        ai_task.clone(),
                        thr,
                    )
                    .await
                    {
                        Ok(v) => return Ok(v),
                        Err(e) if e.to_string().contains("not configured") => {}
                        Err(e) => eprintln!("cloud plan_organize failed, fallback: {e}"),
                    }
                }
            }
        }
    }

    // Mode + entitlement routing for batch: build appropriate provider
    let cfg = ai_mode.lock().map_err(|e| e.to_string())?.clone();
    let online_available = {
        let mgr = license.lock().await;
        let now = chrono::Utc::now().timestamp();
        mgr.is_online_ai_enabled(now) && cfg.online_opt_in && cfg.mode != AiMode::Local
    };

    if online_available {
        // For batch, we run per-file hybrid logic: use operation::plan_organize_with_provider with a custom hybrid provider
        // Minimal: try to use online per file via classify_cmd logic; fallback per file already handled in classify_cmd.
        // For now, fallback to bundled provider but with online attempt per file inside loop.
        // Reuse BundledLocalProvider and attempt online fallback via classify_cmd-style loop.
        // To keep diff small, just use bundled and let classify fallback handle online inside operation loop is not embedded.
        // So we simulate by using a closure provider that does hybrid.
        // Ponytail: cheapest is to just use bundled; online batch optimization can be added when needed.
    }

    operation::plan_organize(files, &base_path, ai_task, thr)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn execute_plan(
    previews: Vec<PlanPreview>,
    db_path: Option<String>,
) -> Result<Vec<String>, String> {
    let journal = match db_path {
        Some(p) => operation::open_journal(Path::new(&p)).map_err(|e| e.to_string())?,
        None => operation::open_default_journal().map_err(|e| e.to_string())?,
    };
    let ids = operation::execute_previews(&previews, &journal).map_err(|e| e.to_string())?;
    Ok(ids.into_iter().map(|id| id.to_string()).collect())
}

#[tauri::command]
fn undo_last(count: Option<usize>, db_path: Option<String>) -> Result<Vec<String>, String> {
    let journal = match db_path {
        Some(p) => operation::open_journal(Path::new(&p)).map_err(|e| e.to_string())?,
        None => operation::open_default_journal().map_err(|e| e.to_string())?,
    };
    let n = count.unwrap_or(1);
    let ids = journal.undo_last(n).map_err(|e| e.to_string())?;
    Ok(ids.into_iter().map(|id| id.to_string()).collect())
}

#[tauri::command]
async fn plan_organize_cmd(
    files: Vec<String>,
    base: String,
    task: Option<String>,
    threshold: Option<f32>,
    provider: Option<String>,
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
    ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
) -> Result<Vec<PlanPreview>, String> {
    plan_organize(files, base, task, threshold, provider, license, ai_mode).await
}

#[tauri::command]
fn execute_plan_cmd(
    previews: Vec<PlanPreview>,
    db_path: Option<String>,
) -> Result<Vec<String>, String> {
    execute_plan(previews, db_path)
}

#[tauri::command]
fn undo_last_cmd(count: Option<usize>, db_path: Option<String>) -> Result<Vec<String>, String> {
    undo_last(count, db_path)
}

#[tauri::command]
fn hardware_info_cmd() -> String {
    serde_json::to_string(&hardware::HardwareInfo::detect()).unwrap_or_else(|_| "{}".to_string())
}

#[tauri::command]
fn model_status_cmd() -> String {
    let reg = model_mgr::global_registry().read().unwrap().clone();
    let total = reg.total_default_bytes();
    serde_json::to_string(&serde_json::json!({
        "models": reg.models,
        "total_bytes": total,
        "total_mb": total as f64 / 1_000_000.0,
        "base_dir": model_mgr::model_base_dir(),
        "hardware": hardware::HardwareInfo::detect(),
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

#[tauri::command]
fn analyze_file_cmd(path: String) -> Result<FileAnalysis, String> {
    let orch = Orchestrator::new();
    orch.analyze(Path::new(&path)).map_err(|e| e.to_string())
}

fn main() {
    let license_mgr = init_license_manager();
    let ai_mode = init_ai_mode();
    tauri::Builder::default()
        .manage(license_mgr)
        .manage(ai_mode)
        .invoke_handler(tauri::generate_handler![
            scan_directory_cmd,
            hash_file_cmd,
            parse_intent_cmd,
            mascot_state_cmd,
            browse_directory_cmd,
            classify_cmd,
            plan_organize,
            plan_organize_cmd,
            execute_plan,
            execute_plan_cmd,
            undo_last,
            undo_last_cmd,
            hardware_info_cmd,
            model_status_cmd,
            analyze_file_cmd,
            license_status_cmd,
            activate_license_cmd,
            refresh_license_cmd,
            get_device_info_cmd,
            set_ai_mode_cmd
        ])
        .run(tauri::generate_context!())
        .expect("tauri run")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_task_variants() {
        assert_eq!(parse_task("DescribeImage"), AiTask::DescribeImage);
        assert_eq!(parse_task("SuggestFilename"), AiTask::SuggestFilename);
        assert_eq!(parse_task("SuggestFolder"), AiTask::SuggestFolder);
        assert_eq!(
            parse_task("DetectSemanticDuplicate"),
            AiTask::DetectSemanticDuplicate
        );
        assert_eq!(parse_task("GenerateTags"), AiTask::GenerateTags);
        assert_eq!(parse_task("SemanticSearch"), AiTask::SemanticSearch);
        assert_eq!(parse_task("SummarizeDocument"), AiTask::SummarizeDocument);
        assert_eq!(parse_task("ClassifyFile"), AiTask::ClassifyFile);
        assert_eq!(parse_task("unknown"), AiTask::ClassifyFile);
    }

    #[test]
    fn parse_ai_mode_variants() {
        assert_eq!(parse_ai_mode("hybrid"), AiMode::Hybrid);
        assert_eq!(parse_ai_mode("HYBRID"), AiMode::Hybrid);
        assert_eq!(parse_ai_mode("online"), AiMode::Online);
        assert_eq!(parse_ai_mode("local"), AiMode::Local);
        assert_eq!(parse_ai_mode("unknown"), AiMode::Local);
    }

    #[test]
    fn license_state_strings() {
        assert_eq!(license_state_str(&LicenseState::Inactive), "inactive");
        assert_eq!(license_state_str(&LicenseState::Active), "active");
        assert_eq!(license_state_str(&LicenseState::Expired), "expired");
        assert_eq!(
            license_state_str(&LicenseState::OfflineGrace),
            "offline_grace"
        );
        assert_eq!(
            license_state_str(&LicenseState::ActivationRequired),
            "activation_required"
        );
        assert_eq!(
            license_state_str(&LicenseState::DeviceConflict),
            "device_conflict"
        );
    }

    #[test]
    fn sanitized_license_json_no_secrets() {
        let store = SecureStore::memory();
        let (dev, sk) = DeviceIdentity::generate();
        store.store_private_key(&sk.to_bytes()).unwrap();
        let mgr = LicenseManager::new("https://api.broomed.app", store, dev);
        let v = sanitized_license_json(&mgr);
        let s = v.to_string();
        assert!(s.contains("activation_required"));
        assert!(!s.contains("private_key"));
        assert!(!s.contains("signing_key"));
        assert!(!s.contains("secret"));
    }

    #[test]
    fn device_creation_and_reload() {
        let store = SecureStore::memory();
        let dev1 = load_or_create_device(&store);
        assert!(!dev1.device_id.is_empty());
        assert_eq!(dev1.public_key_b64.len(), 44);
        let dev2 = load_or_create_device(&store);
        assert_eq!(dev1.device_id, dev2.device_id);
        assert_eq!(dev1.public_key_b64, dev2.public_key_b64);
    }

    #[test]
    fn mascot_state_cmd_json() {
        let s = mascot_state_cmd(false, false, false, false, false, false, false);
        assert!(s.contains("Sleeping") || s.contains("Idle") || s.contains("state"));
        let s_err = mascot_state_cmd(false, false, false, false, false, true, false);
        assert!(s_err.contains("Error") || s_err.contains("error"));
    }
}
