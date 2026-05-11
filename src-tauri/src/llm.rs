// LLM module — first-run model download + inference via bundled llama-server.
//
// Approach:
// - Model GGUF lives in ~/Library/Application Support/ReachOptimizer/models/
// - First-run: download AceGPT-v2-8B Q5_K_M (~5.8GB) with progress events.
// - Inference: spawn `llama-server` (bundled in .app/Contents/MacOS/) and
//   POST to http://127.0.0.1:8765/completion. Server reused across calls.
//
// On Windows dev: the binary path resolution falls back to a stub that returns
// canned text so the UI is testable without the model.

use crate::ClientInput;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

const MODEL_URL: &str = "https://huggingface.co/FreedomIntelligence/AceGPT-v2-8B-Chat-GGUF/resolve/main/AceGPT-v2-8B-Chat-Q5_K_M.gguf";
const MODEL_FILENAME: &str = "acegpt-v2-8b-q5km.gguf";
const MODEL_SIZE_BYTES: u64 = 5_800_000_000; // approximate, used only as fallback

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelStatus {
    pub installed: bool,
    pub path: String,
    pub size_bytes: u64,
}

fn models_dir() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("ReachOptimizer")
        .join("models");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn model_path() -> PathBuf {
    models_dir().join(MODEL_FILENAME)
}

pub async fn status() -> Result<ModelStatus> {
    let p = model_path();
    let installed = p.exists();
    let size = if installed {
        std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    Ok(ModelStatus {
        installed,
        path: p.to_string_lossy().into_owned(),
        size_bytes: size,
    })
}

pub async fn download_with_progress<F>(on_progress: F) -> Result<()>
where
    F: Fn(f64, u64, u64) + Send + 'static,
{
    let target = model_path();
    if target.exists() {
        return Ok(());
    }
    let tmp = target.with_extension("part");

    let client = reqwest::Client::builder()
        .user_agent("ReachOptimizer/0.1")
        .build()?;
    let resp = client.get(MODEL_URL).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(MODEL_SIZE_BYTES);

    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = 0u64;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        file.write_all(&bytes).await?;
        downloaded += bytes.len() as u64;
        if downloaded - last_emit > 5_000_000 {
            let pct = (downloaded as f64 / total as f64) * 100.0;
            on_progress(pct, downloaded, total);
            last_emit = downloaded;
        }
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&tmp, &target).await?;
    on_progress(100.0, downloaded, total);
    Ok(())
}

pub async fn generate_plan(input: &ClientInput) -> Result<String> {
    // TODO: spawn bundled llama-server and POST to it.
    // For now: a deterministic stub that proves the wiring end-to-end.
    let st = status().await?;
    if !st.installed {
        return Err(anyhow!("model not installed"));
    }
    let prompt = build_dialect_prompt(input);
    // Placeholder — replace with real llama-server call:
    Ok(format!(
        "خطة انتشار مبدئية لـ {}\n\nالمنصات: TT={:?}, IG={:?}, SC={:?}\nالنيتش: {}\nالدول: {:?}\nمدة الخطة: {} يوم\n\n[المخرجات الكاملة تُولَّد عند ربط llama-server]\n\nبرومبت اللهجة:\n{}\n",
        input.name,
        input.tiktok,
        input.instagram,
        input.snapchat,
        input.niche,
        input.countries,
        input.plan_days,
        prompt
    ))
}

fn build_dialect_prompt(input: &ClientInput) -> String {
    // النموذج يُوجَّه بأمثلة لهجة خليجية كويتية/سعودية للحصول على نبرة طبيعية.
    let dialect_hint = if input.countries.iter().any(|c| c == "KW") {
        "اكتب بلهجة كويتية ودودة، استخدم: شلون، يبيلك، عاد، حدّه، قاعد."
    } else if input.countries.iter().any(|c| c == "SA") {
        "اكتب بلهجة سعودية حجازية/نجدية حسب السياق، استخدم: كذا، تبي، يا قلبي، ولا يهمك، أبد."
    } else {
        "اكتب بعربي خليجي مفهوم لكل دول الخليج."
    };
    format!(
        "أنت خبير تسويق رقمي خليجي. {}\n\nاقترح خطة انتشار {} يوم لحساب '{}' في مجال {} موجّه لجمهور {:?}.\nاحرص على هاشتاقات حقيقية، أوقات نشر بتوقيت الخليج، وأفكار محتوى عملية.",
        dialect_hint, input.plan_days, input.name, input.niche, input.countries
    )
}
