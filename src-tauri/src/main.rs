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
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use zeroize::Zeroize;

/// Parses an IPC task string into a strongly-typed `AiTask`.
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

#[tauri::command]
fn get_explorer_path_at_point_cmd(x: i32, y: i32) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        return get_explorer_path_at_point_windows(x, y);
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

#[tauri::command]
fn get_cursor_position_cmd() -> (i32, i32, bool) {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct POINT {
            x: i32,
            y: i32,
        }
        extern "system" {
            fn GetCursorPos(lpPoint: *mut POINT) -> i32;
            fn GetAsyncKeyState(vKey: i32) -> i16;
        }
        let mut pt = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut pt);
        }
        let is_down = unsafe { (GetAsyncKeyState(0x01) as u16 & 0x8000) != 0 };
        (pt.x, pt.y, is_down)
    }
    #[cfg(not(target_os = "windows"))]
    {
        (0, 0, false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetWindowInfo {
    pub hwnd: usize,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub folder_name: String,
    pub path: String,
}

#[tauri::command]
fn track_target_window_cmd(x: i32, y: i32) -> Option<TargetWindowInfo> {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct POINT {
            x: i32,
            y: i32,
        }
        #[repr(C)]
        struct RECT {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }
        extern "system" {
            fn WindowFromPoint(point: POINT) -> isize;
            fn GetAncestor(hwnd: isize, gaFlags: u32) -> isize;
            fn GetWindowRect(hwnd: isize, lpRect: *mut RECT) -> i32;
            fn GetClassNameW(hwnd: isize, lpClassName: *mut u16, nMaxCount: i32) -> i32;
            fn GetWindowTextW(hwnd: isize, lpString: *mut u16, nMaxCount: i32) -> i32;
        }

        let pt = POINT { x, y };
        let hwnd = unsafe { WindowFromPoint(pt) };
        if hwnd == 0 {
            return None;
        }
        let root = unsafe {
            let r = GetAncestor(hwnd, 2);
            if r != 0 { r } else { hwnd }
        };

        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        unsafe {
            GetWindowRect(root, &mut rect);
        }

        let width = (rect.right - rect.left).max(0) as u32;
        let height = (rect.bottom - rect.top).max(0) as u32;
        if width < 80 || height < 80 {
            return None;
        }

        let mut cls_buf = [0u16; 256];
        let cls_len = unsafe { GetClassNameW(root, cls_buf.as_mut_ptr(), 256) };
        let cls = String::from_utf16_lossy(&cls_buf[..cls_len.max(0) as usize]);

        let mut title_buf = [0u16; 512];
        let title_len = unsafe { GetWindowTextW(root, title_buf.as_mut_ptr(), 512) };
        let title = String::from_utf16_lossy(&title_buf[..title_len.max(0) as usize]);

        if cls == "Progman" || cls == "WorkerW" {
            let desktop_dir = std::env::var("USERPROFILE")
                .map(|p| format!("{}\\Desktop", p))
                .unwrap_or_else(|_| "Desktop".to_string());
            return Some(TargetWindowInfo {
                hwnd: root as usize,
                x: rect.left,
                y: rect.top,
                width,
                height,
                title: "Desktop".to_string(),
                folder_name: "Desktop".to_string(),
                path: desktop_dir,
            });
        }

        if cls == "CabinetWClass" || cls == "ExploreWClass" {
            let clean_title = title.trim_end_matches(" - File Explorer").trim().to_string();
            let folder_name = if clean_title.is_empty() { "File Explorer".to_string() } else { clean_title.clone() };

            let mut resolved_path = String::new();
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                if folder_name.eq_ignore_ascii_case("downloads") {
                    resolved_path = format!("{}\\Downloads", user_profile);
                } else if folder_name.eq_ignore_ascii_case("documents") {
                    resolved_path = format!("{}\\Documents", user_profile);
                } else if folder_name.eq_ignore_ascii_case("desktop") {
                    resolved_path = format!("{}\\Desktop", user_profile);
                }
            }
            if resolved_path.is_empty() {
                if let Some(p) = get_explorer_path_at_point_windows(x, y) {
                    resolved_path = p;
                }
            }
            if resolved_path.is_empty() {
                resolved_path = folder_name.clone();
            }

            return Some(TargetWindowInfo {
                hwnd: root as usize,
                x: rect.left,
                y: rect.top,
                width,
                height,
                title,
                folder_name,
                path: resolved_path,
            });
        }

        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[tauri::command]
fn show_target_overlay_cmd(
    app: tauri::AppHandle,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    folder_name: String,
) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("target_overlay") {
        let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
        let _ = win.set_size(tauri::Size::Physical(tauri::PhysicalSize { width, height }));
        let _ = win.emit(
            "broomed:update-target-overlay",
            serde_json::json!({
                "folderName": folder_name,
            }),
        );
        let _ = win.show();
    }
    Ok(())
}

#[tauri::command]
fn hide_target_overlay_cmd(app: tauri::AppHandle, confirmed: bool) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("target_overlay") {
        if confirmed {
            let _ = win.emit("broomed:confirm-target-drop", serde_json::json!({}));
            let app_clone = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(350));
                if let Some(w) = app_clone.get_webview_window("target_overlay") {
                    let _ = w.hide();
                }
            });
        } else {
            let _ = win.hide();
        }
    }
    Ok(())
}

#[tauri::command]
fn quit_app_cmd(app: tauri::AppHandle) {
    app.exit(0);
}

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

#[cfg(target_os = "windows")]
fn get_explorer_path_at_point_windows(x: i32, y: i32) -> Option<String> {
    let ps_script = format!(
        r#"
try {{
    Add-Type -MemberDefinition '[DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(System.Drawing.Point p); [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hwnd, uint gaFlags); [DllImport("user32.dll")] public static extern int GetClassName(IntPtr hWnd, System.Text.StringBuilder text, int count);' -Name Win32 -Namespace Temp -ErrorAction SilentlyContinue | Out-Null
    $pt = New-Object System.Drawing.Point({}, {})
    $hwnd = [Temp.Win32]::WindowFromPoint($pt)
    if ($hwnd -ne [IntPtr]::Zero) {{
        $rootHwnd = [Temp.Win32]::GetAncestor($hwnd, 2)
        if ($rootHwnd -eq [IntPtr]::Zero) {{ $rootHwnd = $hwnd }}
        $sb = New-Object System.Text.StringBuilder 256
        [Temp.Win32]::GetClassName($rootHwnd, $sb, 256) | Out-Null
        $cls = $sb.ToString()
        if ($cls -eq 'Progman' -or $cls -eq 'WorkerW') {{
            $desktop = [Environment]::GetFolderPath('Desktop')
            if ($desktop) {{ Write-Output $desktop; exit }}
        }}
        $sh = New-Object -COM Shell.Application
        foreach ($w in $sh.Windows()) {{
            try {{
                if ($w.HWND -eq $rootHwnd.ToInt32() -or $w.HWND -eq $hwnd.ToInt32()) {{
                    $path = $w.Document.Folder.Self.Path
                    if ($path) {{ Write-Output $path; exit }}
                }}
            }} catch {{}}
        }}
    }}
    $sh = New-Object -COM Shell.Application
    foreach ($w in $sh.Windows()) {{
        try {{
            $wx = [int]$w.Left
            $wy = [int]$w.Top
            $ww = [int]$w.Width
            $wh = [int]$w.Height
            if ({} -ge $wx -and {} -le ($wx + $ww) -and {} -ge $wy -and {} -le ($wy + $wh)) {{
                $path = $w.Document.Folder.Self.Path
                if ($path) {{ Write-Output $path; exit }}
            }}
        }} catch {{}}
    }}
}} catch {{}}
"#,
        x, y, x, x, y, y
    );
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &ps_script,
        ])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() && Path::new(&s).exists() {
                return Some(s);
            }
            None
        }
        _ => None,
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

#[tauri::command]
async fn classify_cmd(
    task: String,
    input: String,
    _provider: Option<String>,
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
    ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
) -> Result<AiResult, String> {
    let ai_task = parse_task(&task);
    let cfg = ai_mode.lock().map_err(|e| e.to_string())?.clone();
    classify_auto(ai_task, &input, &license, &cfg).await
}

// ── Phase 2: pipeline commands ───────────────────────────────

#[tauri::command]
async fn plan_organize(
    files: Vec<String>,
    base: String,
    task: Option<String>,
    threshold: Option<f32>,
    _provider: Option<String>,
    license: tauri::State<'_, Arc<tokio::sync::Mutex<LicenseManager>>>,
    ai_mode: tauri::State<'_, Mutex<AiModeConfig>>,
) -> Result<Vec<PlanPreview>, String> {
    let ai_task = parse_task(task.as_deref().unwrap_or("ClassifyFile"));
    let base_path = PathBuf::from(&base);
    let thr = threshold.unwrap_or(0.5);
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
            // System tray with Open / Quit
            let open_item =
                tauri::menu::MenuItem::with_id(app, "open", "Open Broomed", true, None::<&str>)?;
            let quit_item =
                tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&open_item, &quit_item])?;

            let icon = app
                .default_window_icon()
                .cloned()
                .unwrap_or_else(|| tauri::image::Image::new(&[], 0, 0));

            let _tray = tauri::tray::TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("Broomed")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(win) = app.get_webview_window("widget") {
                            let _ = win.show();
                            let _ = win.unminimize();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("widget") {
                            let _ = win.show();
                            let _ = win.unminimize();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
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
            set_ai_mode_cmd,
            get_active_explorer_path_cmd,
            get_explorer_path_at_point_cmd,
            get_cursor_position_cmd,
            track_target_window_cmd,
            show_target_overlay_cmd,
            hide_target_overlay_cmd,
            show_widget_window_cmd,
            hide_widget_window_cmd,
            drag_widget_window_cmd,
            get_user_downloads_dir_cmd,
            resize_widget_cmd,
            save_byok_config_cmd,
            clear_byok_config_cmd,
            get_byok_config_cmd,
            get_active_ai_status_cmd,
            quit_app_cmd
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
