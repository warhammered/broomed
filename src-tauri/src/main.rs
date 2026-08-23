#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use broomed_core::{
    ai::{AiProvider, AiResult, AiTask, BundledLocalProvider, CloudProvider, HeuristicFallback},
    analysis::FileAnalysis,
    bridge, hardware, hash, intent, mascot,
    models as model_mgr,
    operation::{self, PlanPreview},
    orchestrator::Orchestrator,
};

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
    // reuse broomed_core::intent directly; bridge helper also available
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
    // return JSON-friendly string; frontend expects label/animation hint
    serde_json::to_string(&state).unwrap_or_else(|_| format!("{state:?}"))
}

#[tauri::command]
fn browse_directory_cmd() -> Result<Option<String>, String> {
    // ponytail: keep native `<input webkitdirectory>` in frontend — no tauri-plugin-dialog
    // This stub returns None; frontend uses `folder-input` change event instead.
    Ok(None)
}

fn cloud_from_provider_str(s: &str) -> Option<CloudProvider> {
    match s {
        "openai" => Some(CloudProvider::openai()),
        "anthropic" => Some(CloudProvider::anthropic()),
        "cloud" => {
            let o = CloudProvider::openai();
            if o.is_configured() {
                Some(o)
            } else {
                let a = CloudProvider::anthropic();
                if a.is_configured() { Some(a) } else { Some(o) }
            }
        }
        _ => None,
    }
}

#[tauri::command]
async fn classify_cmd(task: String, input: String, provider: Option<String>) -> Result<AiResult, String> {
    let ai_task = parse_task(&task);
    // offline forced heuristic
    if provider.as_deref() == Some("offline") {
        let fallback = HeuristicFallback::new();
        return fallback.classify(ai_task, &input).await.map_err(|e| e.to_string());
    }
    // try cloud if requested
    if let Some(p) = provider.as_deref() {
        if let Some(cp) = cloud_from_provider_str(p) {
            if cp.supports(&ai_task) {
                match cp.classify(ai_task.clone(), &input).await {
                    Ok(r) => return Ok(r),
                    Err(e) if e.to_string().contains("not configured") => {
                        // fall through to bundled
                    }
                    Err(e) => {
                        // ponytail: cloud error fallback to bundled; surface if bundled also fails
                        eprintln!("cloud classify failed, fallback bundled: {e}");
                    }
                }
            }
        }
    }
    // Prefer bundled provider (lazy OnceLock load on first call); fallback to heuristic if model missing
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

// ── Phase 2: pipeline commands (thin, logic in core) ───────────────

#[tauri::command]
async fn plan_organize(
    files: Vec<String>,
    base: String,
    task: Option<String>,
    threshold: Option<f32>,
    provider: Option<String>,
) -> Result<Vec<PlanPreview>, String> {
    let ai_task = parse_task(task.as_deref().unwrap_or("ClassifyFile"));
    let base_path = PathBuf::from(&base);
    let thr = threshold.unwrap_or(0.5);
    // offline shortcut
    if provider.as_deref() == Some("offline") {
        let hf = HeuristicFallback::new();
        return operation::plan_organize_with_provider(files, &base_path, &hf, ai_task, thr)
            .await
            .map_err(|e| e.to_string());
    }
    if let Some(p) = provider.as_deref() {
        if let Some(cp) = cloud_from_provider_str(p) {
            if cp.supports(&ai_task) {
                match operation::plan_organize_with_provider(files.clone(), &base_path, &cp, ai_task.clone(), thr).await {
                    Ok(v) => return Ok(v),
                    Err(e) if e.to_string().contains("not configured") => {
                        // fallback to bundled/heuristic
                    }
                    Err(e) => {
                        eprintln!("cloud plan_organize failed, fallback: {e}");
                    }
                }
            }
        }
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

// Back-compat wrappers keeping snake_cmd names if frontend already uses them
#[tauri::command]
async fn plan_organize_cmd(
    files: Vec<String>,
    base: String,
    task: Option<String>,
    threshold: Option<f32>,
    provider: Option<String>,
) -> Result<Vec<PlanPreview>, String> {
    plan_organize(files, base, task, threshold, provider).await
}

#[tauri::command]
fn execute_plan_cmd(
    previews: Vec<PlanPreview>,
    db_path: Option<String>,
) -> Result<Vec<String>, String> {
    execute_plan(previews, db_path)
}

#[tauri::command]
fn undo_last_cmd(
    count: Option<usize>,
    db_path: Option<String>,
) -> Result<Vec<String>, String> {
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
    tauri::Builder::default()
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
            analyze_file_cmd
        ])
        .run(tauri::generate_context!())
        .expect("tauri run")
}
