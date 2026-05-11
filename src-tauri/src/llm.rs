// LLM lifecycle:
// 1. setup() — runs once. Downloads model GGUF if missing, then spawns the
//    bundled llama-server process pointing at it.
// 2. generate_plan() — sends a chat-completion request to the local server.
//
// All paths are resolved relative to the Tauri AppHandle so the same code
// works in `cargo run` (resources next to crate) and in the .app bundle
// (resources under Contents/Resources/).

use crate::ClientInput;
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

const MODEL_URL: &str = "https://huggingface.co/FreedomIntelligence/AceGPT-v2-8B-Chat-GGUF/resolve/main/AceGPT-v2-8B-Chat-Q5_K_M.gguf";
const MODEL_FILENAME: &str = "acegpt-v2-8b-q5km.gguf";
const MODEL_SIZE_FALLBACK: u64 = 5_800_000_000;
const SERVER_PORT: u16 = 8765;

static SERVER_CHILD: Lazy<Mutex<Option<std::process::Child>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppStatus {
    pub model_installed: bool,
    pub server_running: bool,
    pub model_path: String,
}

fn data_dir() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ReachOptimizer");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn model_path() -> PathBuf {
    let d = data_dir().join("models");
    std::fs::create_dir_all(&d).ok();
    d.join(MODEL_FILENAME)
}

fn llama_dir(app: &AppHandle) -> Result<PathBuf> {
    // In dev: src-tauri/resources/llama. In bundle: Contents/Resources/resources/llama.
    let p = app
        .path()
        .resource_dir()
        .context("no resource_dir")?
        .join("resources/llama");
    Ok(p)
}

fn server_bin(app: &AppHandle) -> Result<PathBuf> {
    Ok(llama_dir(app)?.join("llama-server"))
}

pub fn current_status(_app: &AppHandle) -> AppStatus {
    let mp = model_path();
    let running = SERVER_CHILD.lock().unwrap().is_some();
    AppStatus {
        model_installed: mp.exists(),
        server_running: running,
        model_path: mp.to_string_lossy().into_owned(),
    }
}

pub async fn ensure_model<F>(on_progress: F) -> Result<()>
where
    F: Fn(f64, u64, u64) + Send + 'static,
{
    let target = model_path();
    if target.exists() {
        on_progress(100.0, target.metadata()?.len(), target.metadata()?.len());
        return Ok(());
    }
    let tmp = target.with_extension("part");
    let client = reqwest::Client::builder()
        .user_agent("ReachOptimizer/0.2")
        .build()?;
    let resp = client.get(MODEL_URL).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(MODEL_SIZE_FALLBACK);
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = 0u64;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        file.write_all(&bytes).await?;
        downloaded += bytes.len() as u64;
        if downloaded - last_emit > 5_000_000 {
            on_progress((downloaded as f64 / total as f64) * 100.0, downloaded, total);
            last_emit = downloaded;
        }
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&tmp, &target).await?;
    on_progress(100.0, downloaded, total);
    Ok(())
}

pub fn start_server(app: &AppHandle) -> Result<()> {
    {
        let guard = SERVER_CHILD.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }
    }
    let bin = server_bin(app)?;
    if !bin.exists() {
        return Err(anyhow!(
            "llama-server not bundled at {} — rebuild app",
            bin.display()
        ));
    }
    let mp = model_path();
    if !mp.exists() {
        return Err(anyhow!("model not downloaded yet"));
    }
    let lib_dir = llama_dir(app)?;

    let child = std::process::Command::new(&bin)
        .arg("-m").arg(&mp)
        .arg("--port").arg(SERVER_PORT.to_string())
        .arg("--host").arg("127.0.0.1")
        .arg("-c").arg("4096")
        .arg("-ngl").arg("999")           // offload all layers to Metal
        .arg("--no-webui")
        .env("DYLD_LIBRARY_PATH", &lib_dir)
        .env("DYLD_FALLBACK_LIBRARY_PATH", &lib_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn {}", bin.display()))?;
    *SERVER_CHILD.lock().unwrap() = Some(child);
    Ok(())
}

/// Called from Tauri's exit hook — terminates llama-server cleanly.
pub fn shutdown_server() {
    if let Some(mut child) = SERVER_CHILD.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub async fn wait_until_ready(timeout_secs: u64) -> Result<()> {
    let url = format!("http://127.0.0.1:{}/health", SERVER_PORT);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let client = reqwest::Client::new();
    while std::time::Instant::now() < deadline {
        if let Ok(r) = client.get(&url).send().await {
            if r.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }
    Err(anyhow!("server not ready in {}s", timeout_secs))
}

pub async fn generate_plan(input: &ClientInput) -> Result<String> {
    let prompt = build_dialect_prompt(input);
    let body = serde_json::json!({
        "messages": [
            {"role": "system", "content": "أنت مساعد تسويق رقمي للمحتوى الخليجي."},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.7,
        "max_tokens": 1500,
        "stream": false
    });
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", SERVER_PORT);
    let resp: serde_json::Value = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let text = resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(لا توجد إجابة)")
        .to_string();
    Ok(text)
}

fn build_dialect_prompt(input: &ClientInput) -> String {
    let dialect_hint = if input.countries.iter().any(|c| c == "KW") {
        "اكتب بلهجة كويتية ودودة واستخدم: شلون، يبيلك، عاد، حدّه، قاعد."
    } else if input.countries.iter().any(|c| c == "SA") {
        "اكتب بلهجة سعودية مفهومة (حجازية/نجدية حسب السياق): كذا، تبي، ولا يهمك، أبد."
    } else {
        "اكتب بعربي خليجي مفهوم لكل دول الخليج."
    };
    format!(
        "{}\n\nاقترح خطة انتشار لمدة {} يوم لحساب '{}' في مجال {} موجّه لجمهور {:?}.\n\nأخرج الخطة بصيغة Markdown تتضمن:\n1. ٥ هاشتاقات حقيقية لكل منصة (TikTok, Instagram).\n2. أوقات النشر الذهبية بتوقيت الخليج.\n3. ١٠ أفكار محتوى عملية للشهر.\n4. كلمات بايو SEO مقترحة.\n5. ٣ مؤشرات نجاح يجب متابعتها.",
        dialect_hint, input.plan_days, input.name, input.niche, input.countries
    )
}

#[allow(dead_code)]
fn _path_exists(p: &Path) -> bool { p.exists() }
