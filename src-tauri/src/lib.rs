// ZEED Reach Optimizer — Tauri backend entry point.
mod db;
mod llm;
mod scrapers;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClientInput {
    pub name: String,
    pub tiktok: Option<String>,
    pub instagram: Option<String>,
    pub snapchat: Option<String>,
    pub niche: String,
    pub countries: Vec<String>,
    pub plan_days: u32,
}

#[derive(Debug, Serialize, Clone)]
pub struct ReportSummary {
    pub id: i64,
    pub client_name: String,
    pub generated_at: String,
    pub status: String,
}

pub struct AppState {
    pub db: Mutex<Option<sqlx::SqlitePool>>,
}

#[tauri::command]
async fn check_model_status() -> Result<llm::ModelStatus, String> {
    llm::status().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn download_model(window: tauri::Window) -> Result<(), String> {
    llm::download_with_progress(move |pct, downloaded, total| {
        let _ = window.emit(
            "model:progress",
            serde_json::json!({
                "percent": pct,
                "downloaded": downloaded,
                "total": total,
            }),
        );
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn generate_plan(
    state: State<'_, AppState>,
    input: ClientInput,
) -> Result<ReportSummary, String> {
    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    let id = db::insert_client(pool, &input)
        .await
        .map_err(|e| e.to_string())?;

    // Run scrapers + LLM (stub for now — fills out report)
    let plan_text = llm::generate_plan(&input).await.map_err(|e| e.to_string())?;
    db::save_report(pool, id, &plan_text)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ReportSummary {
        id,
        client_name: input.name,
        generated_at: chrono::Utc::now().to_rfc3339(),
        status: "ready".into(),
    })
}

#[tauri::command]
async fn list_clients(state: State<'_, AppState>) -> Result<Vec<ReportSummary>, String> {
    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    db::list_clients(pool).await.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            db: Mutex::new(None),
        })
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match db::init().await {
                    Ok(pool) => {
                        let state: State<AppState> = handle.state();
                        let mut guard = state.db.lock().await;
                        *guard = Some(pool);
                    }
                    Err(e) => eprintln!("DB init failed: {e}"),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_model_status,
            download_model,
            generate_plan,
            list_clients,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
