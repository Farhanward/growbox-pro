// ZEED Reach Optimizer — Tauri backend.
mod db;
mod insights;
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
    pub content: String,
}

pub struct AppState {
    pub db: Mutex<Option<sqlx::SqlitePool>>,
}

#[tauri::command]
fn app_status(app: tauri::AppHandle) -> llm::AppStatus {
    llm::current_status(&app)
}

#[tauri::command]
async fn setup_app(app: tauri::AppHandle, window: tauri::Window) -> Result<(), String> {
    let w = window.clone();
    let _ = w.emit("setup:progress", serde_json::json!({"stage":"model","percent":0.0}));
    llm::ensure_model(move |pct, dl, total| {
        let _ = w.emit(
            "setup:progress",
            serde_json::json!({"stage":"model","percent":pct,"downloaded":dl,"total":total}),
        );
    })
    .await
    .map_err(|e| format!("فشل تحميل النموذج: {e}"))?;

    let _ = window.emit("setup:progress", serde_json::json!({"stage":"server","percent":0.0}));
    llm::start_server(&app).map_err(|e| format!("فشل تشغيل المحرّك: {e}"))?;
    llm::wait_until_ready(60)
        .await
        .map_err(|e| format!("المحرّك لم يستجب: {e}"))?;
    let _ = window.emit("setup:progress", serde_json::json!({"stage":"ready","percent":100.0}));
    Ok(())
}

/// Full pipeline: scrape → competitors → trends → LLM → save.
#[tauri::command]
async fn generate_report(
    app: tauri::AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
    input: ClientInput,
) -> Result<ReportSummary, String> {
    let w = window.clone();
    let emit = move |stage: &str, msg: &str| {
        let _ = w.emit(
            "report:progress",
            serde_json::json!({"stage": stage, "message": msg}),
        );
    };

    let bundle = insights::gather(&app, &input, emit.clone())
        .await
        .map_err(|e| format!("فشل جمع البيانات: {e}"))?;
    let context = insights::build_context_block(&bundle, &input);

    let content = llm::generate_report(&input, &context)
        .await
        .map_err(|e| format!("فشل توليد التقرير: {e}"))?;

    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    let id = db::insert_client(pool, &input)
        .await
        .map_err(|e| e.to_string())?;
    db::save_report(pool, id, &content)
        .await
        .map_err(|e| e.to_string())?;

    Ok(ReportSummary {
        id,
        client_name: input.name,
        generated_at: chrono::Utc::now().to_rfc3339(),
        status: "ready".into(),
        content,
    })
}

#[tauri::command]
async fn list_clients(state: State<'_, AppState>) -> Result<Vec<ReportSummary>, String> {
    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    db::list_clients(pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_report(state: State<'_, AppState>, id: i64) -> Result<String, String> {
    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    db::load_latest_report(pool, id).await.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState { db: Mutex::new(None) })
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(pool) = db::init().await {
                    let state: State<AppState> = handle.state();
                    *state.db.lock().await = Some(pool);
                }
                let st = llm::current_status(&handle);
                if st.model_installed && !st.server_running {
                    let _ = llm::start_server(&handle);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            setup_app,
            generate_report,
            list_clients,
            load_report,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                llm::shutdown_server();
            }
        });
}
