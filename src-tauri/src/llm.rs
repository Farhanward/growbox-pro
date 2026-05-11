// LLM lifecycle + report generation.
//
// generate_report() now takes a pre-built context string (from insights::gather)
// instead of building one from scratch, so the model sees real numbers.

use crate::ClientInput;
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

// Aya Expanse 8B (Cohere) — multilingual incl. Arabic + Gulf dialects.
// Open-license (CC-BY-NC), no HF auth/token required.
const MODEL_URL: &str = "https://huggingface.co/bartowski/aya-expanse-8b-GGUF/resolve/main/aya-expanse-8b-Q5_K_M.gguf";
const MODEL_FILENAME: &str = "aya-expanse-8b-q5km.gguf";
const MODEL_SIZE_FALLBACK: u64 = 5_803_568_832;
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
    let p = app
        .path()
        .resource_dir()
        .context("no resource_dir")?
        .join("resources/llama");
    Ok(p)
}

fn server_bin(app: &AppHandle) -> Result<PathBuf> {
    let name = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    Ok(llama_dir(app)?.join(name))
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
        let sz = target.metadata()?.len();
        on_progress(100.0, sz, sz);
        return Ok(());
    }
    let tmp = target.with_extension("part");
    let client = reqwest::Client::builder()
        .user_agent("ReachOptimizer/0.3")
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

    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("-m").arg(&mp)
        .arg("--port").arg(SERVER_PORT.to_string())
        .arg("--host").arg("127.0.0.1")
        .arg("-c").arg("8192")
        .arg("--no-webui");
    #[cfg(target_os = "macos")]
    {
        cmd.arg("-ngl").arg("999");
        cmd.env("DYLD_LIBRARY_PATH", &lib_dir);
        cmd.env("DYLD_FALLBACK_LIBRARY_PATH", &lib_dir);
    }
    #[cfg(target_os = "windows")]
    {
        let path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{};{}", lib_dir.display(), path));
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn {}", bin.display()))?;
    *SERVER_CHILD.lock().unwrap() = Some(child);
    Ok(())
}

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

pub async fn generate_report(input: &ClientInput, context: &str) -> Result<String> {
    let dialect_hint = if input.countries.iter().any(|c| c == "KW") {
        "اكتب التقرير بلهجة كويتية ودودة (شلون، يبيلك، عاد، حدّه، قاعد)."
    } else if input.countries.iter().any(|c| c == "SA") {
        "اكتب التقرير بلهجة سعودية مفهومة (كذا، تبي، ولا يهمك، أبد)."
    } else {
        "اكتب التقرير بعربي خليجي مفهوم لكل دول الخليج."
    };
    let system = format!(
        "أنت خبير تسويق رقمي خليجي متخصص في نمو حسابات السوشيال ميديا. \
         تعتمد فقط على البيانات الفعلية المُعطاة لك ولا تختلق أرقاماً. {}",
        dialect_hint
    );
    let user = format!(
        "بناءً على البيانات التالية:\n\n{}\n\n\
         اكتب تقرير انتشار شامل بصيغة Markdown يحتوي على:\n\
         1. **ملخص تنفيذي**: ٣ نقاط تصف وضع الحساب مقارنة بالمنافسين.\n\
         2. **تشخيص البايو والـ SEO**: كلمات مقترحة لتظهر في البحث الداخلي.\n\
         3. **هاشتاقات لكل منشور** (٥ كبيرة + ١٠ متوسطة + ٥ نيتش).\n\
         4. **أوقات النشر الذهبية** بتفصيل لكل دولة مستهدفة.\n\
         5. **١٤ فكرة محتوى** عملية للأسابيع القادمة، كل فكرة بسطر واحد.\n\
         6. **خطة أسبوعية مفصّلة** للأيام السبعة الأولى: متى ينشر وأي محتوى وأي هاشتاقات.\n\
         7. **تحليل المنافسين**: ماذا يفعلون وماذا تستفيد منه.\n\
         8. **مؤشرات النجاح**: ما يجب قياسه بعد ٧ و١٤ و٣٠ يوم.\n\
         9. **توصيات خاصة بالخوارزمية**: ٥ نصائح تقنية لزيادة الـ Reach.\n\n\
         استخدم عناوين Markdown (##) وقوائم نقطية. لا تستخدم أرقام مختلقة.",
        context
    );

    let body = serde_json::json!({
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "temperature": 0.65,
        "max_tokens": 3000,
        "stream": false
    });
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", SERVER_PORT);
    let resp: serde_json::Value = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let text = resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(لا توجد إجابة من النموذج)")
        .to_string();
    Ok(text)
}
