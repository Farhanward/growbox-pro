use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Mutex,
};

const INSTAGRAM_DEBUG_PORT: u16 = 9222;
const TIKTOK_DEBUG_PORT: u16 = 9223;

static BROWSERS: Lazy<Mutex<HashMap<String, BrowserInstance>>> = Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize)]
pub struct IsolatedBrowserStatus {
    pub running: bool,
    pub port: u16,
    pub browser: Option<String>,
    pub profile_dir: String,
    pub profile_label: String,
}

#[derive(Debug, Clone)]
struct BrowserProfile {
    label: String,
    dir: PathBuf,
}

struct BrowserInstance {
    child: Child,
    browser_name: String,
    profile: BrowserProfile,
    port: u16,
}

pub fn status() -> IsolatedBrowserStatus {
    let mut browsers = BROWSERS.lock().unwrap();
    browsers.retain(|_, instance| instance.child.try_wait().map(|status| status.is_none()).unwrap_or(false));
    let first = browsers.values().next();
    let profile = first
        .map(|instance| instance.profile.clone())
        .unwrap_or_else(|| browser_profile("general", None));
    IsolatedBrowserStatus {
        running: first.is_some(),
        port: first.map(|instance| instance.port).unwrap_or(INSTAGRAM_DEBUG_PORT),
        browser: first.map(|instance| instance.browser_name.clone()),
        profile_dir: profile.dir.to_string_lossy().into_owned(),
        profile_label: profile.label,
    }
}

pub fn launch(platform: &str, account_label: Option<&str>) -> Result<IsolatedBrowserStatus> {
    let url = platform_url(platform);
    launch_url(platform, account_label, url)
}

pub fn launch_publish_page(platform: &str, account_label: Option<&str>) -> Result<IsolatedBrowserStatus> {
    let url = publish_url(platform)?;
    launch_url(platform, account_label, url)
}

fn launch_url(platform: &str, account_label: Option<&str>, url: &str) -> Result<IsolatedBrowserStatus> {
    let profile = browser_profile(platform, account_label);
    let key = profile.label.clone();
    let port = debug_port_for_platform(platform)?;
    let mut existing_browser_is_running = false;
    {
        let mut browsers = BROWSERS.lock().unwrap();
        if let Some(instance) = browsers.get_mut(&key) {
            if instance.child.try_wait()?.is_none() {
                existing_browser_is_running = true;
            } else {
                browsers.remove(&key);
            }
        }
    }
    if existing_browser_is_running {
        open_tab(port, url)?;
        return Ok(status());
    }

    let browser = find_browser().ok_or_else(|| anyhow!("لم يتم العثور على Chrome أو Edge على هذا الجهاز."))?;
    std::fs::create_dir_all(&profile.dir)?;

    let mut cmd = Command::new(&browser.path);
    cmd.arg(format!("--remote-debugging-port={port}"))
        .arg(format!("--user-data-dir={}", profile.dir.display()))
        .arg("--no-first-run")
        .arg("--new-window")
        .arg(url);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn().map_err(|e| anyhow!("فشل تشغيل المتصفح المعزول: {e}"))?;
    BROWSERS.lock().unwrap().insert(
        key,
        BrowserInstance {
            child,
            browser_name: browser.name,
            profile,
            port,
        },
    );
    Ok(status())
}

pub fn stop() {
    let mut browsers = BROWSERS.lock().unwrap();
    for (_, mut instance) in browsers.drain() {
        let _ = instance.child.kill();
        let _ = instance.child.wait();
    }
}

fn open_tab(port: u16, url: &str) -> Result<()> {
    let endpoint = format!("http://127.0.0.1:{port}/json/new?{}", urlencoding::encode(url));
    let _ = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?
        .put(endpoint)
        .send();
    Ok(())
}

fn debug_port_for_platform(platform: &str) -> Result<u16> {
    match platform {
        "instagram" => Ok(INSTAGRAM_DEBUG_PORT),
        "tiktok" => Ok(TIKTOK_DEBUG_PORT),
        _ => Err(anyhow!("منصة غير مدعومة")),
    }
}

fn browser_profile(platform: &str, account_label: Option<&str>) -> BrowserProfile {
    let raw_label = account_label
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    let label = format!("{platform}-{raw_label}");
    let slug = stable_profile_slug(&label);
    let dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("GrowBoxPro")
        .join("isolated-browser-profiles")
        .join(slug);
    BrowserProfile { label, dir }
}

fn stable_profile_slug(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.to_lowercase().hash(&mut hasher);
    format!("profile-{:x}", hasher.finish())
}

fn platform_url(platform: &str) -> &'static str {
    match platform {
        "instagram" => "https://www.instagram.com/",
        "tiktok" => "https://www.tiktok.com/",
        _ => "https://www.google.com/",
    }
}

fn publish_url(platform: &str) -> Result<&'static str> {
    match platform {
        "instagram" => Ok("https://www.instagram.com/"),
        "tiktok" => Ok("https://www.tiktok.com/upload"),
        _ => Err(anyhow!("منصة غير مدعومة")),
    }
}

struct BrowserCandidate {
    name: String,
    path: PathBuf,
}

fn find_browser() -> Option<BrowserCandidate> {
    browser_candidates()
        .into_iter()
        .find(|candidate| candidate.path.exists())
}

fn browser_candidates() -> Vec<BrowserCandidate> {
    #[cfg(windows)]
    {
        let program_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
        let program_files_x86 = std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into());
        let local_app = std::env::var("LOCALAPPDATA").unwrap_or_default();
        return vec![
            candidate("Microsoft Edge", Path::new(&program_files).join("Microsoft\\Edge\\Application\\msedge.exe")),
            candidate("Microsoft Edge", Path::new(&program_files_x86).join("Microsoft\\Edge\\Application\\msedge.exe")),
            candidate("Google Chrome", Path::new(&program_files).join("Google\\Chrome\\Application\\chrome.exe")),
            candidate("Google Chrome", Path::new(&program_files_x86).join("Google\\Chrome\\Application\\chrome.exe")),
            candidate("Google Chrome", Path::new(&local_app).join("Google\\Chrome\\Application\\chrome.exe")),
        ];
    }
    #[cfg(target_os = "macos")]
    {
        return vec![
            candidate("Google Chrome", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            candidate("Microsoft Edge", "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
        ];
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return vec![
            candidate("Google Chrome", "/usr/bin/google-chrome"),
            candidate("Chromium", "/usr/bin/chromium"),
            candidate("Microsoft Edge", "/usr/bin/microsoft-edge"),
        ];
    }
}

fn candidate<N, P>(name: N, path: P) -> BrowserCandidate
where
    N: Into<String>,
    P: Into<PathBuf>,
{
    BrowserCandidate { name: name.into(), path: path.into() }
}
