#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use tauri::Manager;

#[tauri::command]
async fn dispatch(input: String) -> String {
    oshwhub_core::dispatch_json(&input).await
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            oshwhub_core::diagnostics::initialize(&app.path().app_log_dir()?)?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "desktop.ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![dispatch])
        .run(tauri::generate_context!())
        .expect("Unable to launch OSHWHub desktop");
}
