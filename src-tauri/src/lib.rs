// ZEED Reach Optimizer — Tauri backend.
mod background;
mod data_fetcher;
mod db;
mod evolution;
mod insights;
mod insights_csv;
mod isolated_browser;
mod license;
mod llm;
mod oauth;
mod preflight;
mod publishing_assistant;
mod resource_manager;
mod vault;
mod vision;

use serde::{Deserialize, Serialize};
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State,
};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostDraftInput {
    pub client: ClientInput,
    pub platform: String,
    pub media_path: Option<String>,
    pub media_note: String,
}

#[tauri::command]
fn app_status(app: tauri::AppHandle) -> llm::AppStatus {
    llm::current_status(&app)
}

#[tauri::command]
fn system_preflight() -> preflight::SystemPreflight {
    preflight::run()
}

#[tauri::command]
fn resource_status() -> resource_manager::ResourceStatus {
    resource_manager::status()
}

#[tauri::command]
fn license_status() -> Result<license::LicenseStatus, String> {
    license::status().map_err(|e| e.to_string())
}

#[tauri::command]
fn activate_license(code: String) -> Result<license::LicenseStatus, String> {
    license::activate_license(&code).map_err(|e| e.to_string())
}

#[tauri::command]
fn start_oauth(platform: String) -> Result<String, String> {
    license::require_active().map_err(|e| e.to_string())?;
    oauth::start_oauth(&platform).map_err(|e| e.to_string())
}

#[tauri::command]
async fn complete_oauth(platform: String, code: String) -> Result<oauth::ConnectedAccount, String> {
    license::require_active().map_err(|e| e.to_string())?;
    oauth::complete_oauth(&platform, &code).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_connected_accounts() -> Result<Vec<oauth::ConnectedAccount>, String> {
    license::require_active().map_err(|e| e.to_string())?;
    oauth::connected_accounts().map_err(|e| e.to_string())
}

#[tauri::command]
async fn collect_account_data(platform: String) -> Result<oauth::AccountData, String> {
    license::require_active().map_err(|e| e.to_string())?;
    oauth::collect_account_data(&platform).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn disconnect_account(platform: String) -> Result<(), String> {
    license::require_active().map_err(|e| e.to_string())?;
    oauth::disconnect(&platform).map_err(|e| e.to_string())
}

#[tauri::command]
fn background_status() -> background::BackgroundStatus {
    background::status()
}

#[tauri::command]
fn evolution_status() -> evolution::EvolutionStatus {
    evolution::status()
}

#[tauri::command]
fn start_background_mode(
    app: tauri::AppHandle,
    window: tauri::Window,
    input: ClientInput,
) -> Result<background::BackgroundStatus, String> {
    license::require_active().map_err(|e| e.to_string())?;
    if input.name.trim().is_empty() {
        return Err("أدخل بيانات العميل قبل تشغيل وضع الخلفية.".into());
    }
    background::start(app, window, input);
    Ok(background::status())
}

#[tauri::command]
fn stop_background_mode() -> background::BackgroundStatus {
    background::stop();
    background::status()
}

#[tauri::command]
async fn run_evolution_report(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<db::StrategicHealthReport, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let _guard = resource_manager::acquire("hermes", "تشغيل Hermes لإنشاء تقرير 7 أيام.").await;
    llm::start_server(&app).map_err(|e| format!("فشل تشغيل المحرّك: {e}"))?;
    llm::wait_until_ready(60)
        .await
        .map_err(|e| format!("المحرّك لم يستجب: {e}"))?;
    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    let result = evolution::run_once(pool).await.map_err(|e| e.to_string());
    llm::shutdown_server();
    result
}

#[tauri::command]
async fn list_evolution_reports(state: State<'_, AppState>) -> Result<Vec<db::StrategicHealthReport>, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    db::list_strategic_health_reports(pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_evolution_tracker(
    app: tauri::AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<evolution::EvolutionStatus, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?.clone();
    Ok(evolution::start(app, pool, window))
}

#[tauri::command]
fn stop_evolution_tracker() -> evolution::EvolutionStatus {
    evolution::stop();
    evolution::status()
}

#[tauri::command]
async fn open_official_account_webview(_app: tauri::AppHandle, platform: String) -> Result<(), String> {
    license::require_active().map_err(|e| e.to_string())?;
    isolated_browser::launch(&platform, None).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn isolated_browser_status() -> isolated_browser::IsolatedBrowserStatus {
    isolated_browser::status()
}

#[tauri::command]
fn launch_isolated_browser(platform: String, account_label: Option<String>) -> Result<isolated_browser::IsolatedBrowserStatus, String> {
    license::require_active().map_err(|e| e.to_string())?;
    isolated_browser::launch(&platform, account_label.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn launch_publish_page(platform: String, account_label: Option<String>) -> Result<isolated_browser::IsolatedBrowserStatus, String> {
    license::require_active().map_err(|e| e.to_string())?;
    isolated_browser::launch_publish_page(&platform, account_label.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn stop_isolated_browser() -> isolated_browser::IsolatedBrowserStatus {
    isolated_browser::stop();
    isolated_browser::status()
}

#[tauri::command]
fn list_login_credentials() -> Result<Vec<vault::LoginCredentialSummary>, String> {
    license::require_active().map_err(|e| e.to_string())?;
    vault::list_credentials().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_login_credential(input: vault::LoginCredentialInput) -> Result<vault::LoginCredentialSummary, String> {
    license::require_active().map_err(|e| e.to_string())?;
    vault::save_credential(input).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_login_credential(id: String) -> Result<(), String> {
    license::require_active().map_err(|e| e.to_string())?;
    vault::delete_credential(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn launch_saved_account_session(id: String) -> Result<isolated_browser::IsolatedBrowserStatus, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let (platform, profile_label) = vault::profile_label_for(&id).map_err(|e| e.to_string())?;
    isolated_browser::launch(&platform, Some(&profile_label)).map_err(|e| e.to_string())
}

#[tauri::command]
fn minimize_to_tray(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
async fn setup_app(_app: tauri::AppHandle, window: tauri::Window) -> Result<(), String> {
    license::require_active().map_err(|e| e.to_string())?;
    let _guard = resource_manager::acquire("setup", "تجهيز نموذج Hermes بدون إبقاء المحرك مفتوحاً.").await;
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

    let _ = window.emit("setup:progress", serde_json::json!({"stage":"ready","percent":100.0}));
    resource_manager::forced_cleanup();
    Ok(())
}

#[tauri::command]
async fn generate_post_draft(
    app: tauri::AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
    input: PostDraftInput,
) -> Result<ReportSummary, String> {
    license::require_active().map_err(|e| e.to_string())?;
    if input.media_note.trim().is_empty() {
        return Err("أدخل وصف الصورة أو الفيديو حتى يستطيع النموذج تجهيز البوست.".into());
    }
    let _guard = resource_manager::acquire("hermes", "تشغيل Hermes لصياغة المسودة النهائية.").await;
    llm::start_server(&app).map_err(|e| format!("فشل تشغيل المحرّك: {e}"))?;
    llm::wait_until_ready(60)
        .await
        .map_err(|e| format!("المحرّك لم يستجب: {e}"))?;

    let w = window.clone();
    let emit = move |stage: &str, msg: &str| {
        let _ = w.emit("report:progress", serde_json::json!({"stage": stage, "message": msg}));
    };

    emit("draft_start", "بدء اعتماد المسودة النهائية…");
    let bundle = insights::gather(&app, &input.client, emit.clone())
        .await
        .map_err(|e| format!("فشل جمع البيانات العامة: {e}"))?;
    emit("draft_context", "تنظيم بيانات الاستراتيجية والوسائط…");
    let mut context = insights::build_context_block(&bundle, &input.client);
    if let Ok(account) = oauth::collect_account_data(&input.platform).await {
        context.push_str("\n## بيانات الحساب المرتبط\n");
        context.push_str(&format!("- {}: {}\n", account.platform, account.summary));
    }
    if let Some(path) = &input.media_path {
        context.push_str("\n## الوسائط المرفقة\n");
        context.push_str(&format!("- ملف محلي مرفق للمراجعة اليدوية: {}\n", path));
    }
    if let Ok(Some(vision_note)) = vision::describe_media(&app, input.media_path.as_deref()).await {
        context.push_str("\n");
        context.push_str(&vision_note);
    }

    emit("draft_llm", "Hermes يكتب النسخة النهائية للبوست…");
    let content = llm::generate_post_draft(&input.client, &context, &input.media_note)
        .await
        .map_err(|e| format!("فشل تجهيز البوست: {e}"))?;

    emit("draft_save", "حفظ المسودة في سجل البوستات…");
    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    let id = db::insert_client(pool, &input.client).await.map_err(|e| e.to_string())?;
    db::save_report(pool, id, &content).await.map_err(|e| e.to_string())?;
    emit("draft_ready", "تم تجهيز المسودة. افتح صفحة النشر الرسمية لإكمال الرفع.");
    llm::shutdown_server();

    Ok(ReportSummary {
        id,
        client_name: input.client.name,
        generated_at: chrono::Utc::now().to_rfc3339(),
        status: "ready".into(),
        content,
    })
}

#[tauri::command]
async fn analyze_publish_strategy(
    app: tauri::AppHandle,
    window: tauri::Window,
    input: publishing_assistant::PublishingAssistantInput,
) -> Result<publishing_assistant::PublishingAssistantOutput, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let _guard = resource_manager::acquire("hermes", "تشغيل Hermes لتحليل الاستراتيجية.").await;
    llm::start_server(&app).map_err(|e| format!("فشل تشغيل المحرّك: {e}"))?;
    llm::wait_until_ready(60)
        .await
        .map_err(|e| format!("المحرّك لم يستجب: {e}"))?;
    let w = window.clone();
    let emit = move |stage: &str, msg: &str| {
        let _ = w.emit("report:progress", serde_json::json!({"stage": stage, "message": msg}));
    };

    let result = publishing_assistant::analyze(&app, input, emit)
        .await
        .map_err(|e| format!("فشل مساعد النشر الذكي: {e}"));
    llm::shutdown_server();
    result
}

/// Playwright CDP fetch: connect to isolated browser, intercept API, return normalized insights.
#[tauri::command]
async fn fetch_post_insights(
    app: tauri::AppHandle,
    platform: String,
    post_url: String,
) -> Result<data_fetcher::CapturedInsights, String> {
    license::require_active().map_err(|e| e.to_string())?;
    if post_url.trim().is_empty() {
        return Err("أدخل رابط المنشور.".into());
    }
    data_fetcher::fetch(&app, &platform, &post_url)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn discover_competitor_posts(
    app: tauri::AppHandle,
    platform: String,
    hashtags: Vec<String>,
) -> Result<Vec<String>, String> {
    license::require_active().map_err(|e| e.to_string())?;
    data_fetcher::discover_posts(&app, &platform, &hashtags)
        .await
        .map_err(|e| format!("فشل الاستكشاف التلقائي: {e}"))
}

#[tauri::command]
async fn vision_status() -> serde_json::Value {
    vision::status()
}

#[tauri::command]
async fn inspect_media_with_vision(
    app: tauri::AppHandle,
    window: tauri::Window,
    media_path: String,
) -> Result<vision::VisionInspection, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let _guard = resource_manager::acquire("vision", "تحرير الذاكرة وتشغيل Qwen2-VL لقراءة الوسائط.").await;
    llm::shutdown_server();
    let result = vision::inspect_media(&app, &window, Some(&media_path))
        .await
        .map_err(|e| format!("فشل تحليل الوسائط: {e}"));
    resource_manager::forced_cleanup();
    result
}

#[tauri::command]
async fn download_vision_model(window: tauri::Window) -> Result<serde_json::Value, String> {
    license::require_active().map_err(|e| e.to_string())?;
    vision::download_model(window)
        .await
        .map_err(|e| format!("فشل تحميل النموذج البصري: {e}"))
}

#[tauri::command]
async fn analyze_with_market_evidence(
    app: tauri::AppHandle,
    window: tauri::Window,
    input: publishing_assistant::PublishingAssistantInput,
    fetched: data_fetcher::CapturedInsights,
    competitors: Vec<data_fetcher::CapturedInsights>,
) -> Result<publishing_assistant::PublishingAssistantOutput, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let _guard = resource_manager::acquire("hermes", "تحرير الذاكرة وتشغيل Hermes لصياغة البطاقات بالأدلة.").await;
    llm::start_server(&app).map_err(|e| format!("فشل تشغيل المحرّك: {e}"))?;
    llm::wait_until_ready(60)
        .await
        .map_err(|e| format!("المحرّك لم يستجب: {e}"))?;
    let w = window.clone();
    let emit = move |stage: &str, msg: &str| {
        let _ = w.emit("report:progress", serde_json::json!({"stage": stage, "message": msg}));
    };
    let result = publishing_assistant::analyze_with_market_evidence(&app, input, fetched, competitors, emit)
        .await
        .map_err(|e| format!("فشل التحليل بالأدلة الفعلية: {e}"));
    llm::shutdown_server();
    result
}

/// Full pipeline with fetched insights injected as real-number context.
#[tauri::command]
async fn analyze_with_fetched_insights(
    app: tauri::AppHandle,
    window: tauri::Window,
    input: publishing_assistant::PublishingAssistantInput,
    fetched: data_fetcher::CapturedInsights,
) -> Result<publishing_assistant::PublishingAssistantOutput, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let _guard = resource_manager::acquire("hermes", "تشغيل Hermes للتحليل المعزز بالإحصاءات.").await;
    llm::start_server(&app).map_err(|e| format!("فشل تشغيل المحرّك: {e}"))?;
    llm::wait_until_ready(60)
        .await
        .map_err(|e| format!("المحرّك لم يستجب: {e}"))?;
    let w = window.clone();
    let emit = move |stage: &str, msg: &str| {
        let _ = w.emit("report:progress", serde_json::json!({"stage": stage, "message": msg}));
    };
    let result = publishing_assistant::analyze_with_insights(&app, input, fetched, emit)
        .await
        .map_err(|e| format!("فشل التحليل المُعزَّز: {e}"));
    llm::shutdown_server();
    result
}

/// Full pipeline: official/local signals → niche references → trends → LLM → save.
#[tauri::command]
async fn generate_report(
    app: tauri::AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
    input: ClientInput,
) -> Result<ReportSummary, String> {
    license::require_active().map_err(|e| e.to_string())?;
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

/// Parse one or more CSV files exported from Meta Business Suite Insights.
/// Returns a merged InsightsImport structure for the UI to display.
#[tauri::command]
async fn import_insights_csvs(paths: Vec<String>) -> Result<insights_csv::InsightsImport, String> {
    if paths.is_empty() {
        return Err("لم يتم اختيار ملفات.".into());
    }
    let mut merged = insights_csv::InsightsImport::default();
    let mut errors: Vec<String> = vec![];
    for p in &paths {
        match insights_csv::parse_csv(std::path::Path::new(p)) {
            Ok(part) => insights_csv::merge(&mut merged, part),
            Err(e) => errors.push(format!("{}: {}", p, e)),
        }
    }
    merged.source_file = paths.join(", ");
    if merged.daily.is_empty() && merged.top_posts.is_empty()
        && merged.demographics.age_buckets.is_empty()
        && merged.demographics.gender.is_empty()
        && merged.online_hours.is_empty()
    {
        return Err(format!("لم يتم استخراج أي بيانات. التفاصيل:\n{}", errors.join("\n")));
    }
    Ok(merged)
}

/// Generate a report enriched with imported Insights data (real numbers).
#[tauri::command]
async fn generate_report_with_insights(
    app: tauri::AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
    input: ClientInput,
    insights: insights_csv::InsightsImport,
) -> Result<ReportSummary, String> {
    license::require_active().map_err(|e| e.to_string())?;
    let w = window.clone();
    let emit = move |stage: &str, msg: &str| {
        let _ = w.emit("report:progress", serde_json::json!({"stage": stage, "message": msg}));
    };

    let bundle = insights::gather(&app, &input, emit.clone())
        .await
        .map_err(|e| format!("فشل جمع البيانات العامة: {e}"))?;
    let mut context = insights::build_context_block(&bundle, &input);
    context.push_str("\n");
    context.push_str(&insights_csv::to_context_block(&insights));

    let content = llm::generate_report(&input, &context)
        .await
        .map_err(|e| format!("فشل توليد التقرير: {e}"))?;

    let pool_guard = state.db.lock().await;
    let pool = pool_guard.as_ref().ok_or("DB not initialized")?;
    let id = db::insert_client(pool, &input).await.map_err(|e| e.to_string())?;
    db::save_report(pool, id, &content).await.map_err(|e| e.to_string())?;

    Ok(ReportSummary {
        id, client_name: input.name,
        generated_at: chrono::Utc::now().to_rfc3339(),
        status: "ready".into(),
        content,
    })
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
            if let Some(icon) = app.default_window_icon().cloned() {
                let _ = TrayIconBuilder::new()
                    .icon(icon)
                    .tooltip("GrowBox Pro")
                    .show_menu_on_left_click(false)
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click { button, button_state, .. } = event {
                            if button == MouseButton::Left && button_state == MouseButtonState::Up {
                                if let Some(window) = tray.app_handle().get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    })
                    .build(app);
            }
            let handle = app.handle().clone();
            oauth::start_callback_server(handle.clone());
            tauri::async_runtime::spawn(async move {
                if let Ok(pool) = db::init().await {
                    let state: State<AppState> = handle.state();
                    *state.db.lock().await = Some(pool);
                }
                let _ = llm::current_status(&handle);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            system_preflight,
            resource_status,
            license_status,
            activate_license,
            setup_app,
            start_oauth,
            complete_oauth,
            get_connected_accounts,
            collect_account_data,
            disconnect_account,
            background_status,
            evolution_status,
            run_evolution_report,
            list_evolution_reports,
            start_evolution_tracker,
            stop_evolution_tracker,
            start_background_mode,
            stop_background_mode,
            isolated_browser_status,
            launch_isolated_browser,
            launch_publish_page,
            stop_isolated_browser,
            list_login_credentials,
            save_login_credential,
            delete_login_credential,
            launch_saved_account_session,
            open_official_account_webview,
            minimize_to_tray,
            vision_status,
            inspect_media_with_vision,
            download_vision_model,
            fetch_post_insights,
            discover_competitor_posts,
            analyze_with_market_evidence,
            analyze_with_fetched_insights,
            analyze_publish_strategy,
            generate_post_draft,
            generate_report,
            generate_report_with_insights,
            import_insights_csvs,
            list_clients,
            load_report,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if matches!(event, tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }) {
                background::stop();
                isolated_browser::stop();
                llm::shutdown_server();
            }
        });
}
