#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use broomed_core::{
    ai::{AiProvider, AiTask, BundledLocalProvider, HeuristicFallback},
    bridge,
    device::DeviceIdentity,
    hardware,
    intent,
    license::LicenseManager,
    mode::AiModeConfig,
    models as model_mgr,
    operation::{self, PlanPreview},
    secure_store::SecureStore,
};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

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

fn load_or_create_device(store: &SecureStore) -> DeviceIdentity {
    if let (Some(device_id), Some(pk_b64)) = (
        store.load_token("device_id"),
        store.load_token("device_pubkey"),
    ) {
        if store.load_private_key().is_some() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
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

fn init_license_manager() -> Mutex<LicenseManager> {
    let api_base =
        std::env::var("BROOMED_API_BASE").unwrap_or_else(|_| "https://api.broomed.app".to_string());
    let store = SecureStore::default();
    let device = load_or_create_device(&store);
    let mgr = LicenseManager::new(api_base, store, device);
    Mutex::new(mgr)
}

fn init_ai_mode() -> Mutex<AiModeConfig> {
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
fn parse_intent_cmd(text: String) -> String {
    format!("{:?}", intent::parse_intent(&text))
}

#[tauri::command]
<<<<<<< HEAD
fn browse_directory_cmd() -> Result<Option<String>, String> {
    Ok(None)
}

=======
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
async fn browse_directory_cmd() -> Result<Option<String>, String> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Select Directory to Organize")
        .pick_folder()
        .await;

    if let Some(folder) = handle {
        let path_str = folder.path().to_string_lossy().to_string();
        if !path_str.is_empty() {
            return Ok(Some(path_str));
        }
    }
    Ok(None)
}

// ── BYOK (Bring Your Own Key) & AI Tier Management ─────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ByokConfig {
    pub provider: String, // "openai", "anthropic", "openrouter", "custom"
    pub api_key: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ByokConfigResponse {
    pub configured: bool,
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub has_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAiStatus {
    pub tier: String, // "pro_online" | "byok" | "local"
    pub label: String, // "✨ Pro Online AI" | "🔑 Custom LLM" | "⚡ Local AI (BERT)"
    pub details: String,
    pub credits_remaining: Option<i64>,
}

fn load_byok_config() -> Option<ByokConfig> {
    let store = SecureStore::new();
    if let Some(raw) = store.load_token("byok_config_json") {
        if let Ok(cfg) = serde_json::from_str::<ByokConfig>(&raw) {
            if !cfg.api_key.trim().is_empty() {
                return Some(cfg);
            }
        }
    }
    // Check fallback individual tokens or env
    if let Some(k) = store.load_token("byok_api_key") {
        if !k.trim().is_empty() {
            let p = store.load_token("byok_provider").unwrap_or_else(|| "openai".to_string());
            let m = store.load_token("byok_model");
            let b = store.load_token("byok_base_url");
            return Some(ByokConfig {
                provider: p,
                api_key: k,
                model: m,
                base_url: b,
            });
        }
    }
    // Dev env fallback
    if let Ok(k) = std::env::var("OPENAI_API_KEY") {
        if !k.trim().is_empty() {
            return Some(ByokConfig {
                provider: "openai".into(),
                api_key: k,
                model: Some("gpt-4o-mini".into()),
                base_url: None,
            });
        }
    }
    if let Ok(k) = std::env::var("ANTHROPIC_API_KEY") {
        if !k.trim().is_empty() {
            return Some(ByokConfig {
                provider: "anthropic".into(),
                api_key: k,
                model: Some("claude-3-5-sonnet-20241022".into()),
                base_url: None,
            });
        }
    }
    None
}

#[tauri::command]
fn save_byok_config_cmd(
    provider: String,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
) -> Result<(), String> {
    let store = SecureStore::new();
    let cfg = ByokConfig {
        provider: provider.trim().to_lowercase(),
        api_key: api_key.trim().to_string(),
        model: model.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        base_url: base_url.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
    };
    let raw = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
    store.store_token("byok_config_json", &raw)?;
    Ok(())
}

#[tauri::command]
fn clear_byok_config_cmd() -> Result<(), String> {
    let store = SecureStore::new();
    let _ = store.store_token("byok_config_json", "");
    let _ = store.store_token("byok_api_key", "");
    Ok(())
}

#[tauri::command]
fn get_byok_config_cmd() -> Result<ByokConfigResponse, String> {
    if let Some(cfg) = load_byok_config() {
        let default_model = if cfg.provider == "anthropic" {
            "claude-3-5-sonnet-20241022"
        } else {
            "gpt-4o-mini"
        };
        Ok(ByokConfigResponse {
            configured: !cfg.api_key.trim().is_empty(),
            provider: cfg.provider,
            model: cfg.model.unwrap_or_else(|| default_model.to_string()),
            base_url: cfg.base_url,
            has_key: !cfg.api_key.trim().is_empty(),
        })
    } else {
        Ok(ByokConfigResponse {
            configured: false,
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            base_url: None,
            has_key: false,
        })
    }
}

#[tauri::command]
async fn get_active_ai_status_cmd(
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
) -> Result<ActiveAiStatus, String> {
    let now = chrono::Utc::now().timestamp();
    let mgr = license.lock().await;
    if mgr.is_online_ai_enabled(now) {
        let credits = mgr.entitlement.as_ref().and_then(|e| e.ai_credits_remaining);
        return Ok(ActiveAiStatus {
            tier: "pro_online".into(),
            label: "Pro Online AI".into(),
            details: "Broomed Cloud Reasoning Gateway".into(),
            credits_remaining: credits,
        });
    }
    drop(mgr);

    if let Some(byok) = load_byok_config() {
        let prov_name = match byok.provider.as_str() {
            "anthropic" => "Anthropic",
            "openrouter" => "OpenRouter",
            "custom" => "Custom LLM",
            _ => "OpenAI",
        };
        let m = byok.model.unwrap_or_else(|| "gpt-4o-mini".into());
        return Ok(ActiveAiStatus {
            tier: "byok".into(),
            label: format!("BYOK ({prov_name})"),
            details: format!("{prov_name} • {m}"),
            credits_remaining: None,
        });
    }

    Ok(ActiveAiStatus {
        tier: "local".into(),
        label: "Local AI (BERT)".into(),
        details: "Offline 384-dim embedding model".into(),
        credits_remaining: None,
    })
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

#[tauri::command]
fn get_active_explorer_path_cmd() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        return get_active_explorer_path_windows();
    }
    #[cfg(target_os = "macos")]
    {
        return get_active_explorer_path_macos();
    }
    #[cfg(target_os = "linux")]
    {
        return get_active_explorer_path_linux();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
#[tauri::command]
fn show_main_window_cmd(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.show().map_err(|e| e.to_string())?;
        win.unminimize().map_err(|e| e.to_string())?;
        win.set_focus().map_err(|e| e.to_string())?;
    } else {
        return Err("main window not found".to_string());
    }
    Ok(())
}

#[tauri::command]
fn get_active_explorer_path_cmd() -> Option<String> {
    None
}

#[tauri::command]
fn emit_plan_to_main_cmd(
    app: tauri::AppHandle,
    folder_path: String,
    previews: Vec<PlanPreview>,
) -> Result<(), String> {
    let payload = serde_json::json!({ "folderPath": folder_path, "previews": previews });
    let _ = app.emit("broomed:plan-ready", payload);
    Ok(())
}

<<<<<<< HEAD
=======
#[cfg(target_os = "windows")]
fn get_active_explorer_path_windows() -> Option<String> {
    // Windows implementation (primary, dev is on win32):
    // Uses PowerShell COM fallback without requiring `windows` crate.
    // TODO: Full COM IShellWindows enumeration via `windows` crate for robust
    // foreground detection (GetForegroundWindow + GetWindowTextW + IShellWindows).
    // For now, PowerShell approach is lightweight and returns None gracefully on failure.
    let ps_script = r#"
try {
    Add-Type -MemberDefinition '[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr hWnd, System.Text.StringBuilder text, int count);' -Name Win32 -Namespace Temp -ErrorAction SilentlyContinue | Out-Null
    $hwnd = [Temp.Win32]::GetForegroundWindow()
    $sh = New-Object -COM Shell.Application
    foreach ($w in $sh.Windows()) {
        try {
            if ($w.HWND -eq $hwnd.ToInt32()) {
                $path = $w.Document.Folder.Self.Path
                if ($path) { Write-Output $path; exit }
            }
        } catch {}
    }
    foreach ($w in $sh.Windows()) {
        try {
            $path = $w.Document.Folder.Self.Path
            if ($path) { Write-Output $path; exit }
        } catch {}
    }
} catch {}
"#;
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            ps_script,
        ])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && Path::new(&s).exists() {
                return Some(s);
            }
            if !s.is_empty() {
                return Some(s);
            }
            eprintln!("get_active_explorer_path_windows: powershell returned empty");
            None
        }
        Ok(out) => {
            eprintln!(
                "get_active_explorer_path_windows: powershell failed status {:?} stderr: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr)
            );
            None
        }
        Err(e) => {
            eprintln!("get_active_explorer_path_windows: failed to spawn powershell: {e}");
            None
        }
    }
}

#[cfg(target_os = "macos")]
fn get_active_explorer_path_macos() -> Option<String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "Finder" to if exists Finder window 1 then get POSIX path of (target of Finder window 1 as alias)"#)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        Ok(out) => {
            eprintln!(
                "get_active_explorer_path_macos: osascript failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            None
        }
        Err(e) => {
            eprintln!("get_active_explorer_path_macos: spawn failed: {e}");
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn get_active_explorer_path_linux() -> Option<String> {
    // Try xdotool + D-Bus, else return None gracefully
    let try_xdotool = std::process::Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output();
    if let Ok(out) = try_xdotool {
        if out.status.success() {
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if name.starts_with('/') && Path::new(&name).exists() {
                return Some(name);
            }
            eprintln!(
                "get_active_explorer_path_linux: xdotool window name: {name} (not a path, returning None)"
            );
        }
    }
    // Attempt D-Bus for Nautilus/Dolphin could be added here (gdbus/qdbus)
    eprintln!("get_active_explorer_path_linux: no supported file manager path found (xdotool/D-Bus fallback)");
    None
}

// ── AI Automatic Tiered Routing ─────────────────────────────

async fn classify_auto(
    ai_task: AiTask,
    input: &str,
    license_mgr: &Arc<tokio::sync::Mutex<LicenseManager>>,
    _ai_mode_cfg: &AiModeConfig,
) -> Result<AiResult, String> {
    // ── Tier 1: Pro Online AI Gateway (if user purchased Pro subscription)
    let now = chrono::Utc::now().timestamp();
    let (is_pro, api_base, token) = {
        let mgr = license_mgr.lock().await;
        let pro = mgr.is_online_ai_enabled(now);
        let token = mgr.entitlement.as_ref().map(|e| e.license_id.clone());
        let api = mgr.api_base.clone();
        (pro, api, token)
    };

    if is_pro {
        if let Some(tok) = token {
            #[cfg(feature = "online-ai")]
            {
                let cap = match ai_task {
                    AiTask::DescribeImage => "vision",
                    _ => "text",
                };
                let url = format!("{}/api/ai/{}", api_base.trim_end_matches('/'), cap);
                let http = reqwest::Client::new();
                let payload = serde_json::json!({
                    "capability": cap,
                    "input": input,
                    "task": format!("{:?}", ai_task)
                });
                if let Ok(resp) = http
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", tok))
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
                        eprintln!("[Broomed] Pro Online AI returned status {} — falling back to BYOK/Local", resp.status());
                    }
                }
            }
        }
    }

    // ── Tier 2: BYOK (Bring Your Own Key)
    if let Some(byok) = load_byok_config() {
        if !byok.api_key.trim().is_empty() {
            let mut cp = match byok.provider.to_lowercase().as_str() {
                "anthropic" => CloudProvider::anthropic(),
                "openrouter" => CloudProvider::openai().with_base_url("https://openrouter.ai/api/v1"),
                _ => CloudProvider::openai(),
            };
            cp = cp.with_api_key(Some(byok.api_key.clone()));
            if let Some(m) = byok.model.as_deref().filter(|s| !s.trim().is_empty()) {
                cp = cp.with_model(m);
            }
            if let Some(url) = byok.base_url.as_deref().filter(|s| !s.trim().is_empty()) {
                cp = cp.with_base_url(url);
            }
            if cp.supports(&ai_task) {
                match cp.classify(ai_task.clone(), input).await {
                    Ok(r) => return Ok(r),
                    Err(e) => {
                        eprintln!("[Broomed] BYOK error/rate-limited ({e}) — falling back to Local AI");
                    }
                }
            }
        }
    }

    // ── Tier 3: Local AI Model (all-MiniLM-L6-v2 BERT embeddings) -> Heuristic fallback
    let bundled = BundledLocalProvider::new();
    if bundled.supports(&ai_task) {
        if let Ok(res) = bundled.classify(ai_task.clone(), input).await {
            return Ok(res);
        }
    }

    let fallback = HeuristicFallback::new();
    fallback.classify(ai_task, input).await.map_err(|e| e.to_string())
}

>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
#[tauri::command]
async fn classify_cmd(
    task: String,
    input: String,
    _provider: Option<String>,
<<<<<<< HEAD
    _ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
) -> Result<broomed_core::ai::AiResult, String> {
    let ai_task = parse_task(&task);
    let bundled = BundledLocalProvider::new();
    if bundled.supports(&ai_task) {
        if !bundled.model_available() {
            eprintln!(
                "[Broomed] BundledLocalProvider model not found (checked bundled resources and {:?}), using heuristic fallback. CWD={:?}",
                model_mgr::model_dir_for("all-MiniLM-L6-v2"),
                std::env::current_dir().unwrap_or_default()
            );
        }
        let res = bundled
            .classify(ai_task, &input)
            .await
            .map_err(|e| e.to_string())?;
        if res.reason.contains("heuristic") && bundled.model_available() {
            eprintln!("[Broomed] classify used heuristic despite model present — check local-ai feature enabled");
        }
        return Ok(res);
    }
    HeuristicFallback::new()
        .classify(ai_task, &input)
        .await
        .map_err(|e| e.to_string())
=======
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
    ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
) -> Result<AiResult, String> {
    let ai_task = parse_task(&task);
    let cfg = ai_mode.lock().map_err(|e| e.to_string())?.clone();
    classify_auto(ai_task, &input, &license, &cfg).await
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
}

#[tauri::command]
async fn plan_organize(
    files: Vec<String>,
    base: String,
    task: Option<String>,
    threshold: Option<f32>,
<<<<<<< HEAD
    provider: Option<String>,
    _license: tauri::State<'_, Mutex<LicenseManager>>,
    _ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
=======
    _provider: Option<String>,
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
    ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
) -> Result<Vec<PlanPreview>, String> {
    let ai_task = parse_task(task.as_deref().unwrap_or("ClassifyFile"));
    let base_path = PathBuf::from(&base);
    let thr = threshold.unwrap_or(0.5);
<<<<<<< HEAD

    if provider.as_deref() == Some("offline") || provider.as_deref() == Some("local") {
        let hb = HeuristicFallback::new();
        return operation::plan_organize_with_provider(files, &base_path, &hb, ai_task, thr)
            .await
            .map_err(|e| e.to_string());
    }
    if provider.as_deref() == Some("bundled") {
        let bundled = BundledLocalProvider::new();
        if !bundled.model_available() {
            eprintln!(
                "[Broomed] plan_organize bundled model not found (checked {:?}), using heuristic fallback",
                model_mgr::model_dir_for("all-MiniLM-L6-v2")
            );
        }
        return operation::plan_organize_with_provider(files, &base_path, &bundled, ai_task, thr)
            .await
            .map_err(|e| e.to_string());
    }
    operation::plan_organize(files, &base_path, ai_task, thr)
        .await
        .map_err(|e| e.to_string())
=======
    let cfg = ai_mode.lock().map_err(|e| e.to_string())?.clone();

    // Check if we should use local batch or classify per file
    let mut previews = Vec::new();
    for f in &files {
        let r = classify_auto(ai_task.clone(), f, &license, &cfg).await.unwrap_or_else(|_| AiResult {
            category: "General".into(),
            confidence: 0.5,
            suggested_folder: Some("General".into()),
            reason: "Heuristic fallback".into(),
            tags: vec![],
            subcategory: None,
            suggested_name: None,
        });

        let file_name = Path::new(f).file_name().and_then(|n| n.to_str()).unwrap_or(f);
        let folder = r.suggested_folder.clone().unwrap_or_else(|| r.category.clone());
        let dest = base_path.join(&folder).join(file_name);

        previews.push(PlanPreview {
            operation: operation::Operation {
                id: broomed_core::types::OperationId::new(),
                source: PathBuf::from(f),
                destination: dest,
                kind: operation::OpKind::Move,
                reason: r.reason.clone(),
                confidence: r.confidence,
                reversible: true,
                status: "planned".to_string(),
            },
            ai_result: r,
        });
    }

    if thr > 0.0 {
        previews.retain(|p| p.operation.confidence >= thr);
    }

    Ok(previews)
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
}

#[tauri::command]
fn execute_plan_cmd(
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
fn undo_last_cmd(count: Option<usize>, db_path: Option<String>) -> Result<Vec<String>, String> {
    let journal = match db_path {
        Some(p) => broomed_core::operation::open_journal(Path::new(&p)).map_err(|e| e.to_string())?,
        None => broomed_core::operation::open_default_journal().map_err(|e| e.to_string())?,
    };
    let n = count.unwrap_or(1);
    let ids = journal.undo_last(n).map_err(|e| e.to_string())?;
    Ok(ids.into_iter().map(|id| id.to_string()).collect())
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

fn focus_main(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn resize_widget_cmd(
    app: tauri::AppHandle,
    width: Option<f64>,
    height: Option<f64>,
    x_offset: Option<f64>,
    open: Option<bool>,
) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("widget") {
        let target_w = width.unwrap_or(260.0);
        let target_h = if let Some(h) = height {
            h
        } else if let Some(is_open) = open {
            if is_open { 235.0 } else { 180.0 }
        } else {
            180.0
        };
        win.set_size(tauri::LogicalSize::new(target_w, target_h))
            .map_err(|e| e.to_string())?;

        if let Some(dx) = x_offset {
            if dx != 0.0 {
                let scale = win.scale_factor().unwrap_or(1.0);
                if let Ok(pos) = win.outer_position() {
                    let phys_dx = (dx * scale).round() as i32;
                    let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                        pos.x + phys_dx,
                        pos.y,
                    )));
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn show_widget_window_cmd(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("widget") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
    Ok(())
}

#[tauri::command]
fn hide_widget_window_cmd(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("widget") {
        let _ = win.hide();
    }
    Ok(())
}

#[tauri::command]
fn drag_widget_window_cmd(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("widget") {
        let _ = win.start_dragging();
    }
    Ok(())
}

#[tauri::command]
fn get_user_downloads_dir_cmd() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let p = Path::new(&profile).join("Downloads");
            if p.exists() {
                return Ok(p.to_string_lossy().to_string());
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let p = Path::new(&home).join("Downloads");
            if p.exists() {
                return Ok(p.to_string_lossy().to_string());
            }
        }
    }
    Ok(std::env::temp_dir().to_string_lossy().to_string())
}

fn main() {
    let license_mgr = init_license_manager();
    let ai_mode = init_ai_mode();
    tauri::Builder::default()
        .manage(license_mgr)
        .manage(ai_mode)
        .setup(|app| {
            // Hide main window on close instead of quitting
            if let Some(main_win) = app.get_webview_window("main") {
                let win_clone = main_win.clone();
                main_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win_clone.hide();
                    }
                });
            }

            // System tray with Open / Quit
            let open_item =
                tauri::menu::MenuItem::with_id(app, "open", "Open Broomed", true, None::<&str>)?;
            let quit_item =
                tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&open_item, &quit_item])?;

            let icon = app
                .default_window_icon()
                .cloned()
                .expect("window icon missing");

            let _tray = tauri::tray::TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Broomed")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => focus_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        focus_main(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_directory_cmd,
            parse_intent_cmd,
            browse_directory_cmd,
            classify_cmd,
            plan_organize,
            execute_plan_cmd,
            undo_last_cmd,
            model_status_cmd,
            get_active_explorer_path_cmd,
            show_main_window_cmd,
<<<<<<< HEAD
            emit_plan_to_main_cmd,
=======
            hide_main_window_cmd,
            show_widget_window_cmd,
            hide_widget_window_cmd,
            drag_widget_window_cmd,
            get_user_downloads_dir_cmd,
            emit_plan_to_main_cmd,
            resize_widget_cmd,
            save_byok_config_cmd,
            clear_byok_config_cmd,
            get_byok_config_cmd,
            get_active_ai_status_cmd
>>>>>>> 27381596d3bff0b1a26b8941f4429928eb36fc85
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
}
