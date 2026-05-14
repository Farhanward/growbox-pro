use crate::{db, llm, resource_manager};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Emitter, Window};

const SEVEN_DAYS_SECS: u64 = 7 * 24 * 60 * 60;

static RUNNING: AtomicBool = AtomicBool::new(false);
static STATUS: Lazy<Mutex<EvolutionStatus>> = Lazy::new(|| Mutex::new(EvolutionStatus::default()));

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionStatus {
    pub active: bool,
    pub last_run_at: Option<String>,
    pub last_summary: String,
    pub next_check_days: u32,
}

impl Default for EvolutionStatus {
    fn default() -> Self {
        Self {
            active: false,
            last_run_at: None,
            last_summary: "Evolution Tracker متوقف.".into(),
            next_check_days: 7,
        }
    }
}

pub fn status() -> EvolutionStatus {
    let mut status = STATUS.lock().unwrap().clone();
    status.active = RUNNING.load(Ordering::Relaxed);
    status
}

pub fn stop() {
    RUNNING.store(false, Ordering::Relaxed);
    let mut status = STATUS.lock().unwrap();
    status.active = false;
    status.last_summary = "تم إيقاف Evolution Tracker.".into();
}

pub fn start(app: AppHandle, pool: SqlitePool, window: Window) -> EvolutionStatus {
    if RUNNING.swap(true, Ordering::Relaxed) {
        return status();
    }
    {
        let mut status = STATUS.lock().unwrap();
        status.active = true;
        status.last_summary = "Evolution Tracker يعمل: تقرير صحة الاستراتيجية كل 7 أيام.".into();
    }
    let pool = Arc::new(pool);
    tauri::async_runtime::spawn(async move {
        while RUNNING.load(Ordering::Relaxed) {
            let summary = match run_once_with_runtime(&app, &pool).await {
                Ok(report) => {
                    let _ = window.emit("evolution:report", &report);
                    "تم إنشاء تقرير صحة الاستراتيجية.".to_string()
                }
                Err(e) => format!("تعذر إنشاء تقرير صحة الاستراتيجية: {e}"),
            };
            {
                let mut status = STATUS.lock().unwrap();
                status.active = RUNNING.load(Ordering::Relaxed);
                status.last_run_at = Some(chrono::Utc::now().to_rfc3339());
                status.last_summary = summary;
            }
            let _ = window.emit("evolution:status", status());

            for _ in 0..SEVEN_DAYS_SECS {
                if !RUNNING.load(Ordering::Relaxed) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
        let _ = window.emit("evolution:status", status());
    });
    status()
}

pub async fn run_once(pool: &SqlitePool) -> Result<db::StrategicHealthReport> {
    let reports = db::load_recent_report_contents(pool, 12).await?;
    if reports.is_empty() {
        return Err(anyhow!("لا توجد مسودات أو تقارير محفوظة للمقارنة."));
    }
    let mut context = String::new();
    for (name, created_at, content) in reports {
        context.push_str(&format!(
            "\n## {} — {}\n{}\n",
            name,
            created_at,
            content.chars().take(2500).collect::<String>()
        ));
    }
    let content = llm::generate_strategy_health_report(&context).await?;
    let id = db::save_strategic_health_report(pool, &content).await?;
    Ok(db::StrategicHealthReport {
        id,
        generated_at: chrono::Utc::now().to_rfc3339(),
        content,
    })
}

async fn run_once_with_runtime(app: &AppHandle, pool: &SqlitePool) -> Result<db::StrategicHealthReport> {
    let _guard = resource_manager::acquire("hermes", "تشغيل Hermes لتقرير تطور الحساب.").await;
    llm::start_server(app)?;
    llm::wait_until_ready(60).await?;
    let result = run_once(pool).await;
    llm::shutdown_server();
    result
}
