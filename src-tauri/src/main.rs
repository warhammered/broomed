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
fn browse_directory_cmd() -> Result<Option<String>, String> {
    Ok(None)
}

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

#[tauri::command]
async fn classify_cmd(
    task: String,
    input: String,
    _provider: Option<String>,
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
}

#[tauri::command]
async fn plan_organize(
    files: Vec<String>,
    base: String,
    task: Option<String>,
    threshold: Option<f32>,
    provider: Option<String>,
    _license: tauri::State<'_, Mutex<LicenseManager>>,
    _ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
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
            emit_plan_to_main_cmd,
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
