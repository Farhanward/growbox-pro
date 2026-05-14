use crate::{insights, ClientInput};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Emitter, Window};

static RUNNING: AtomicBool = AtomicBool::new(false);
static STATUS: Lazy<Mutex<BackgroundStatus>> = Lazy::new(|| Mutex::new(BackgroundStatus::default()));

#[derive(Debug, Clone, Serialize)]
pub struct BackgroundStatus {
    pub active: bool,
    pub last_run_at: Option<String>,
    pub last_summary: String,
    pub next_check_minutes: u32,
}

impl Default for BackgroundStatus {
    fn default() -> Self {
        Self {
            active: false,
            last_run_at: None,
            last_summary: "وضع الخلفية متوقف.".into(),
            next_check_minutes: 15,
        }
    }
}

pub fn status() -> BackgroundStatus {
    let mut status = STATUS.lock().unwrap().clone();
    status.active = RUNNING.load(Ordering::Relaxed);
    status
}

pub fn stop() {
    RUNNING.store(false, Ordering::Relaxed);
    let mut status = STATUS.lock().unwrap();
    status.active = false;
    status.last_summary = "تم إيقاف وضع الخلفية.".into();
}

pub fn start(app: AppHandle, window: Window, input: ClientInput) {
    if RUNNING.swap(true, Ordering::Relaxed) {
        return;
    }
    {
        let mut status = STATUS.lock().unwrap();
        status.active = true;
        status.last_summary = "وضع الخلفية يعمل: سيتم تحديث إشارات الترند بهدوء.".into();
    }

    tauri::async_runtime::spawn(async move {
        while RUNNING.load(Ordering::Relaxed) {
            let w = window.clone();
            let emit = move |stage: &str, msg: &str| {
                let _ = w.emit("background:progress", serde_json::json!({"stage": stage, "message": msg}));
            };

            let summary = match insights::gather(&app, &input, emit).await {
                Ok(bundle) => format!(
                    "آخر تحديث: {} هاشتاق، {} منافس مرجعي، {} حساب رسمي/محلي.",
                    bundle.trending_hashtags.len(),
                    bundle.competitor_handles.len(),
                    bundle.client_profiles.len()
                ),
                Err(e) => format!("تعذر تحديث إشارات الخلفية: {e}"),
            };

            {
                let mut status = STATUS.lock().unwrap();
                status.active = RUNNING.load(Ordering::Relaxed);
                status.last_run_at = Some(chrono::Utc::now().to_rfc3339());
                status.last_summary = summary.clone();
            }
            let _ = window.emit("background:status", status());

            for _ in 0..30 {
                if !RUNNING.load(Ordering::Relaxed) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        }
        let _ = window.emit("background:status", status());
    });
}
