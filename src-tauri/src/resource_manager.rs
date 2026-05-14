use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

static HEAVY_MODEL_LOCK: Lazy<Arc<AsyncMutex<()>>> = Lazy::new(|| Arc::new(AsyncMutex::new(())));
static STATUS: Lazy<Mutex<ResourceStatus>> = Lazy::new(|| Mutex::new(ResourceStatus::default()));

#[derive(Debug, Clone, Serialize)]
pub struct ResourceStatus {
    pub busy: bool,
    pub current_stage: String,
    pub message: String,
}

impl Default for ResourceStatus {
    fn default() -> Self {
        Self {
            busy: false,
            current_stage: "idle".into(),
            message: "جاهز.".into(),
        }
    }
}

pub struct HeavyModelGuard {
    _guard: OwnedMutexGuard<()>,
}

impl Drop for HeavyModelGuard {
    fn drop(&mut self) {
        forced_cleanup();
        let mut status = STATUS.lock().unwrap();
        status.busy = false;
        status.current_stage = "idle".into();
        status.message = "تم تحرير الموارد.".into();
    }
}

pub async fn acquire(stage: &str, message: &str) -> HeavyModelGuard {
    {
        let mut status = STATUS.lock().unwrap();
        status.busy = true;
        status.current_stage = stage.into();
        status.message = message.into();
    }
    let guard = HEAVY_MODEL_LOCK.clone().lock_owned().await;
    HeavyModelGuard { _guard: guard }
}

pub fn status() -> ResourceStatus {
    STATUS.lock().unwrap().clone()
}

pub fn forced_cleanup() {
    std::thread::sleep(std::time::Duration::from_millis(120));
}
