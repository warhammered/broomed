#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;

use broomed_core::{bridge, hash, intent, mascot};

// ponytail: Tauri replaces pywebview when `tauri dev/build` passes â€” keep core reuse thin, add plugins only when needed

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
    // ponytail: Tauri v1 api::dialog removed in v2 â€” stub returns None, replace with tauri-plugin-dialog when added (tauri::api::dialog::blocking::FileDialogBuilder::pick_folder() was v1)
    Ok(None)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scan_directory_cmd,
            hash_file_cmd,
            parse_intent_cmd,
            mascot_state_cmd,
            browse_directory_cmd
        ])
        .run(tauri::generate_context!())
        .expect("tauri run")
}
