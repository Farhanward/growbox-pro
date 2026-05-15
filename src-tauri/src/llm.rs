// LLM lifecycle + report generation.
//
// generate_report() now takes a pre-built context string (from insights::gather)
// instead of building one from scratch, so the model sees real numbers.

use crate::{preflight, ClientInput};
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

// Hermes 2 Pro Llama 3 8B is tuned for structured JSON and tool-like output.
// GGUF source verified from the NousResearch Hugging Face repository.
const MODEL_URL: &str = "https://huggingface.co/NousResearch/Hermes-2-Pro-Llama-3-8B-GGUF/resolve/main/Hermes-2-Pro-Llama-3-8B-Q5_K_M.gguf";
const MODEL_FILENAME: &str = "hermes-2-pro-llama-3-8b-q5km.gguf";
const MODEL_URL_Q4: &str = "https://huggingface.co/NousResearch/Hermes-2-Pro-Llama-3-8B-GGUF/resolve/main/Hermes-2-Pro-Llama-3-8B-Q4_K_M.gguf";
const MODEL_FILENAME_Q4: &str = "hermes-2-pro-llama-3-8b-q4km.gguf";
const MODEL_SIZE_FALLBACK: u64 = 5_734_000_000;
const SERVER_PORT: u16 = 8765;

static SERVER_CHILD: Lazy<Mutex<Option<std::process::Child>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppStatus {
    pub model_installed: bool,
    pub server_running: bool,
    pub model_path: String,
}

fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let new_dir = base.join("GrowBoxPro");
    // Migrate old "ReachOptimizer" data folder if user upgraded from v0.3.x.
    let old_dir = base.join("ReachOptimizer");
    if old_dir.exists() && !new_dir.exists() {
        let _ = std::fs::rename(&old_dir, &new_dir);
    }
    std::fs::create_dir_all(&new_dir).ok();
    new_dir
}

async fn chat(system: String, user: String, max_tokens: u32) -> Result<String> {
    let body = serde_json::json!({
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "temperature": 0.65,
        "max_tokens": max_tokens,
        "stream": false
    });
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", SERVER_PORT);
    let resp: serde_json::Value = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1800))
        .build()?
        .post(&url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("(لا توجد إجابة من النموذج)")
        .to_string())
}

async fn chat_json(system: String, user: String, max_tokens: u32) -> Result<String> {
    let body = serde_json::json!({
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ],
        "temperature": 0.25,
        "max_tokens": max_tokens,
        "stream": false,
        "response_format": { "type": "json_object" }
    });
    let url = format!("http://127.0.0.1:{}/v1/chat/completions", SERVER_PORT);
    let resp: serde_json::Value = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1800))
        .build()?
        .post(&url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}")
        .to_string())
}

pub fn calculate_strategic_score(hook_strength: &str, shareability_factor: &str, trend_alignment: &str) -> u8 {
    let hook = virality_level_score(hook_strength);
    let shareability = virality_level_score(shareability_factor);
    let trend = trend_alignment_score(trend_alignment);
    ((hook as f32 * 0.45) + (shareability as f32 * 0.35) + (trend as f32 * 0.20)).round() as u8
}

fn virality_level_score(value: &str) -> u8 {
    match value.trim().to_ascii_lowercase().as_str() {
        "high" | "top 10%" => 100,
        "medium" | "top 25%" | "top 40%" => 60,
        "low" | "needs work" => 30,
        _ => 30,
    }
}

fn trend_alignment_score(value: &str) -> u8 {
    match value.trim().to_ascii_lowercase().as_str() {
        "peaking" | "high" => 100,
        "emerging" => 80,
        "rising" | "medium" => 60,
        "stable" | "low" => 30,
        _ => 30,
    }
}

fn model_choice() -> (&'static str, &'static str) {
    if std::env::var("GROWBOX_HERMES_HIGH_QUALITY").ok().as_deref() == Some("1")
        || preflight::run().recommended_profile == "high-q5-vision"
    {
        (MODEL_URL, MODEL_FILENAME)
    } else {
        (MODEL_URL_Q4, MODEL_FILENAME_Q4)
    }
}

fn model_path() -> PathBuf {
    let d = data_dir().join("models");
    std::fs::create_dir_all(&d).ok();
    let (_, filename) = model_choice();
    d.join(filename)
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
        .user_agent("GrowBox/0.4")
        .build()?;
    let (url, _) = model_choice();
    let resp = client.get(url).send().await?.error_for_status()?;
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

    chat(system, user, 2200).await
}

pub async fn generate_post_draft(input: &ClientInput, context: &str, media_note: &str, output_language: &str) -> Result<String> {
    let english_output = is_english_output(output_language);
    let dialect_hint = if english_output {
        "Write the final post in English only. Do not include Arabic copy unless it is a hashtag already provided in the context."
    } else if input.countries.iter().any(|c| c == "KW") {
        "اكتب بلهجة كويتية ودودة ومفهومة فقط، بدون فصحى رسمية."
    } else if input.countries.iter().any(|c| c == "SA") {
        "اكتب بلهجة سعودية خليجية مفهومة ومهنية فقط، بدون فصحى رسمية."
    } else {
        "اكتب بعربي خليجي واضح فقط، بدون فصحى رسمية."
    };
    let system = format!(
        "أنت كاتب محتوى وتسويق سوشيال ميديا. مهمتك تجهيز بوست فقط للنشر اليدوي. \
         ممنوع أن تقول إنك ستنشر أو ترفع المحتوى. لا تختلق أرقامًا. \
         إذا كانت اللغة إنجليزية فاكتب البوست بالإنجليزية. إذا كانت عربية فاستخدم لهجة خليجية طبيعية فقط. \
         ممنوع اللهجة المصرية وممنوع اختراع مدينة أو فندق أو علامة تجارية غير مذكورة في السياق. {}",
        dialect_hint
    );
    let template = if english_output {
        "Prepare the final post in Markdown with only these sections:\n\
         ## Ready Caption\n\
         One copy-ready English caption.\n\n\
         ## Hashtags\n\
         5 to 8 strong hashtags only. No hashtag stuffing.\n\n\
         ## Suggested Posting Time\n\
         One or two times maximum based on the data.\n\n\
         ## Notes Before Publishing\n\
         3 short notes to improve the media, first two seconds, or call to action."
    } else {
        "جهز بوست نهائي بصيغة Markdown يحتوي فقط على:\n\
         ## الكابشن الجاهز\n\
         نص واحد جاهز للنسخ بلهجة خليجية فقط.\n\n\
         ## الهاشتاقات\n\
         5 إلى 8 هاشتاقات فقط، منتقاة من الأقوى والأقرب للمحتوى. ممنوع حشو هاشتاقات عامة.\n\n\
         ## وقت النشر المقترح\n\
         وقت واحد أو وقتين كحد أقصى حسب البيانات.\n\n\
         ## ملاحظات للعميل قبل النشر\n\
         3 ملاحظات قصيرة لتحسين الصورة/الفيديو أو أول ثانيتين أو الدعوة للتفاعل."
    };
    let final_rules = if english_output {
        "Do not add Arabic marketing copy. Do not mention any city, hotel, or brand unless it is explicitly present in the media description or context. Do not add a long report or weekly plan."
    } else {
        "لا تكتب بالإنجليزية. لا تذكر مدينة أو فندقاً أو علامة تجارية غير موجودة صراحة في وصف الوسائط أو السياق. \
         ممنوع اللهجة المصرية مثل: ده، دي، بتاع، أوي، عشان كده، تستعديش. \
         لا تضف تقريرًا طويلًا ولا خطة أسبوعية."
    };
    let user = format!(
        "بيانات الحساب والتحليل:\n{}\n\n\
         وصف الصورة أو الفيديو من العميل:\n{}\n\n\
         {}\n\n\
         {}",
        context,
        media_note.trim(),
        template,
        final_rules
    );
    chat(system, user, 900).await
}

pub async fn generate_strategy_health_report(context: &str) -> Result<String> {
    let system = "أنت مستشار نمو استراتيجي لحسابات السوشيال ميديا. تعمل محلياً، وتلتزم فقط بالبيانات التاريخية المعطاة. لا تخترع أرقاماً أو نتائج.".to_string();
    let user = format!(
        "هذه آخر المسودات والتقارير المحفوظة في GrowBox:\n\n{}\n\n\
         اكتب Strategic Health Report بصيغة Markdown يحتوي على:\n\
         ## ملخص صحة الاستراتيجية\n\
         3 نقاط عن اتجاه الأداء بناءً على النصوص المتاحة فقط.\n\n\
         ## إشارات النمو أو الضعف\n\
         استنتج الأنماط المتكررة في الكابشن، الهاشتاقات، وقت النشر، والنبرة.\n\n\
         ## Pivot أو استمرار\n\
         قرر هل يحتاج المسار الحالي إلى تعديل Pivot أم استمرار، مع سبب واضح.\n\n\
         ## مؤشر الانحراف Deviation Index\n\
         قيّم الفجوة بين هاشتاقات/نبرة/موضوعات المستخدم وبين نمط النجاح المذكور داخل التقارير. \
         إذا تجاوزت الفجوة 30% بحسب الأدلة النصية المتاحة، اكتب: تغيير مسار إلزامي، ثم 3 خطوات تصحيحية. \
         إذا كانت بيانات المنافسين غير موجودة، قل إن المؤشر غير محسوب بدقة ولا تخترع نسبة.\n\n\
         ## تجربة الأسبوع القادم\n\
         5 توصيات عملية قابلة للقياس خلال 7 أيام.\n\n\
         إذا كانت البيانات غير كافية، قل ذلك بوضوح ولا تضف أرقاماً.",
        context
    );
    chat(system, user, 1200).await
}

pub async fn generate_publishing_assistant(
    input: &ClientInput,
    platform: &str,
    context: &str,
    media_note: &str,
    allowed_hashtags: &[String],
    suggested_time: &str,
    output_language: &str,
) -> Result<String> {
    let english_output = is_english_output(output_language);
    let language_rule = if english_output {
        "All user-facing post copy, captions, summaries, rationale, cards, and guardrails must be in clear English. Do not write Arabic captions when output_language is en."
    } else if input.countries.iter().any(|c| c == "KW") {
        "اكتب الكابشن وكل النصوص التسويقية العربية بلهجة كويتية مهنية وخفيفة فقط، وممنوع الفصحى الرسمية أو اللهجة المصرية."
    } else if input.countries.iter().any(|c| c == "SA") {
        "اكتب الكابشن وكل النصوص التسويقية العربية بلهجة سعودية خليجية مهنية وواضحة فقط، وممنوع الفصحى الرسمية أو اللهجة المصرية."
    } else {
        "اكتب الكابشن وكل النصوص التسويقية العربية بعربي خليجي مفهوم فقط، وممنوع الفصحى الرسمية أو اللهجة المصرية."
    };
    let target_region = if input.countries.iter().any(|c| c == "SA") {
        "Saudi Arabia"
    } else {
        "Gulf Countries"
    };
    let system = format!(
        "أنت 'خبير نمو رقمي' متخصص في السوق السعودي والخليجي داخل GrowBox Pro. \
         مهمتك اقتراح استراتيجية قبل النشر بصيغة JSON فقط، وليس تنفيذ النشر. \
         اعمل كمحرك مطابقة وتحليل ساكن Static Analysis Mapping، وليس كمحرك تخمين. \
         اربط وصف المحتوى والسياق والوسوم المسموحة بأنماط أداء ناجحة في المنطقة، ولا تستنتج حدثاً أو ترنداً غير موجود في البيانات المدخلة. \
         إذا كانت output_language=en فاكتب البوست بالإنجليزية. إذا كانت output_language=ar فاكتب البوست بالعربية الخليجية فقط. \
         ممنوع اللهجة المصرية في العربية: ده، دي، بتاع، أوي، تستعديش. ممنوع اختراع مدينة أو فندق أو علامة تجارية غير موجودة صراحة في وصف Vision أو السياق. \
         استخدم فقط الهاشتاقات الموجودة في allowed_hashtags، ولا تضف ترندات خارج النيتش أو الدول المحددة. \
         عند استخراج virality_indicators التزم بالآتي: \
         hook_strength يطابق الجملة الأولى أو العناصر المرئية الموصوفة مع الفضول المعرفي، حل المشكلة الفوري، أو الوعد بالقيمة؛ لا تستخدم High إلا عند وجود لهجة محلية قوية أو حاجة ملحة في السوق السعودي. \
         shareability_factor يبحث عن القيمة المتبادلة: تعليمي، ملهم، فكاهي سياقي، هوية مجتمعية، نصيحة عملية، تعبير عن الذات، أو وسم وطني/تقني نشط. \
         predicted_trend_alignment يطابق الكلمات المفتاحية مع موجات السعودية والخليج مثل رؤية 2030، التحول التقني، والفعاليات الموسمية في الرياض/جدة؛ استخدم Peaking فقط إذا ظهر دليل صريح على حدث جارٍ الآن في المنطقة المستهدفة. \
         إذا لم تجد دليلاً منطقياً على القوة، استخدم Medium أو Low ولا ترفع التقييم. \
         ممنوع Markdown، ممنوع شرح خارج JSON، وممنوع نص قبل أو بعد الكائن. {}",
        language_rule
    );
    let user = format!(
        "target_region: {}\n\
         output_language: {}\n\
         analysis_mode: Evidence Based Strategy Mapping\n\n\
         السياق المحلي:\n{}\n\n\
         وصف الوسائط:\n{}\n\n\
         allowed_hashtags: {}\n\
         suggested_time: {}\n\n\
         أعد JSON مطابقاً لهذا schema فقط وبكل الحقول:\n\
         {{\
           \"platform_info\": {{\"name\": \"{}\", \"post_id\": \"GBX-DRAFT\"}},\
           \"niche_context\": {{\"account_type\": \"{}\", \"region\": \"{}\"}},\
           \"content_payload\": {{\
             \"caption\": \"نص الكابشن المقترح\",\
             \"hashtags\": {{\"velocity\": [\"#tag\"], \"relevance\": [\"#tag\"]}}\
           }},\
           \"strategy_insights\": {{\
             \"success_score\": 75,\
             \"posting_time\": \"{}\",\
             \"reasoning\": \"سبب تحليلي مختصر\"\
           }},\
           \"virality_indicators\": {{\
             \"hook_strength\": \"High | Medium | Low\",\
             \"shareability_factor\": \"High | Medium | Low\",\
             \"predicted_trend_alignment\": \"Peaking | Emerging | Rising | Stable\",\
             \"strategic_score\": 82\
           }}\
         }}\n\n\
         يجب أن تبني reasoning وsuccess_score على الأدلة الرقمية الموجودة في السياق: إحصاءات المنشور الفعلية ومنشورات المنافسين إن وجدت. \
         إذا كانت output_language=en فاكتب caption وsummary وreasoning وvisual_note بالإنجليزية. إذا كانت ar فاكتبها بالعربية الخليجية فقط. \
         إذا لم توجد أرقام كافية، اخفض success_score واذكر في reasoning أن الدليل الرقمي ناقص. \
         strategic_score تحسبه طبقة Rust نهائياً من الأوزان 45% Hook و35% Shareability و20% Trend، لذلك أعده كرقم فقط ولا تشرح المعادلة. \
         لا تستخدم أي هاشتاق غير موجود في allowed_hashtags. لا تضع شرحاً خارج JSON.",
        target_region,
        if english_output { "en" } else { "ar" },
        context,
        media_note.trim(),
        allowed_hashtags.join(" "),
        suggested_time,
        platform,
        input.niche,
        input.countries.join(","),
        suggested_time
    );

    chat_json(system, user, 850).await
}

fn is_english_output(language: &str) -> bool {
    language.trim().eq_ignore_ascii_case("en") || language.trim().eq_ignore_ascii_case("english")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_english_output ───────────────────────────────────────────────────────

    #[test]
    fn english_output_en_lowercase() {
        assert!(is_english_output("en"));
    }

    #[test]
    fn english_output_en_uppercase() {
        assert!(is_english_output("EN"));
    }

    #[test]
    fn english_output_en_mixed_case() {
        assert!(is_english_output("En"));
    }

    #[test]
    fn english_output_english_word() {
        assert!(is_english_output("english"));
    }

    #[test]
    fn english_output_english_uppercase() {
        assert!(is_english_output("ENGLISH"));
    }

    #[test]
    fn english_output_en_with_whitespace() {
        assert!(is_english_output("  en  "));
    }

    #[test]
    fn english_output_ar_returns_false() {
        assert!(!is_english_output("ar"));
    }

    #[test]
    fn english_output_arabic_locale_returns_false() {
        assert!(!is_english_output("ar-SA"));
    }

    #[test]
    fn english_output_empty_returns_false() {
        assert!(!is_english_output(""));
    }

    #[test]
    fn english_output_random_string_returns_false() {
        assert!(!is_english_output("fr"));
        assert!(!is_english_output("de"));
        assert!(!is_english_output("العربية"));
    }

    // ── virality_level_score ────────────────────────────────────────────────────

    #[test]
    fn virality_high_scores_100() {
        assert_eq!(virality_level_score("High"), 100);
        assert_eq!(virality_level_score("high"), 100);
        assert_eq!(virality_level_score("HIGH"), 100);
    }

    #[test]
    fn virality_top_10_scores_100() {
        assert_eq!(virality_level_score("Top 10%"), 100);
        assert_eq!(virality_level_score("top 10%"), 100);
    }

    #[test]
    fn virality_medium_scores_60() {
        assert_eq!(virality_level_score("Medium"), 60);
        assert_eq!(virality_level_score("medium"), 60);
    }

    #[test]
    fn virality_top_25_scores_60() {
        assert_eq!(virality_level_score("Top 25%"), 60);
    }

    #[test]
    fn virality_top_40_scores_60() {
        assert_eq!(virality_level_score("Top 40%"), 60);
    }

    #[test]
    fn virality_low_scores_30() {
        assert_eq!(virality_level_score("Low"), 30);
        assert_eq!(virality_level_score("low"), 30);
    }

    #[test]
    fn virality_needs_work_scores_30() {
        assert_eq!(virality_level_score("Needs Work"), 30);
        assert_eq!(virality_level_score("needs work"), 30);
    }

    #[test]
    fn virality_unknown_falls_back_to_30() {
        assert_eq!(virality_level_score("unknown"), 30);
        assert_eq!(virality_level_score(""), 30);
        assert_eq!(virality_level_score("garbage"), 30);
    }

    // ── trend_alignment_score ───────────────────────────────────────────────────

    #[test]
    fn trend_peaking_scores_100() {
        assert_eq!(trend_alignment_score("Peaking"), 100);
        assert_eq!(trend_alignment_score("peaking"), 100);
    }

    #[test]
    fn trend_high_scores_100() {
        assert_eq!(trend_alignment_score("High"), 100);
    }

    #[test]
    fn trend_emerging_scores_80() {
        assert_eq!(trend_alignment_score("Emerging"), 80);
        assert_eq!(trend_alignment_score("emerging"), 80);
    }

    #[test]
    fn trend_rising_scores_60() {
        assert_eq!(trend_alignment_score("Rising"), 60);
        assert_eq!(trend_alignment_score("rising"), 60);
    }

    #[test]
    fn trend_medium_scores_60() {
        assert_eq!(trend_alignment_score("Medium"), 60);
    }

    #[test]
    fn trend_stable_scores_30() {
        assert_eq!(trend_alignment_score("Stable"), 30);
        assert_eq!(trend_alignment_score("stable"), 30);
    }

    #[test]
    fn trend_low_scores_30() {
        assert_eq!(trend_alignment_score("Low"), 30);
    }

    #[test]
    fn trend_unknown_falls_back_to_30() {
        assert_eq!(trend_alignment_score("Viral"), 30);
        assert_eq!(trend_alignment_score(""), 30);
    }

    // ── calculate_strategic_score ───────────────────────────────────────────────

    #[test]
    fn strategic_score_all_max() {
        // hook=100*0.45 + share=100*0.35 + trend=100*0.20 = 100
        assert_eq!(calculate_strategic_score("High", "High", "Peaking"), 100);
    }

    #[test]
    fn strategic_score_all_min() {
        // hook=30*0.45 + share=30*0.35 + trend=30*0.20 = 13.5+10.5+6 = 30
        assert_eq!(calculate_strategic_score("Low", "Low", "Stable"), 30);
    }

    #[test]
    fn strategic_score_all_medium() {
        // hook=60*0.45 + share=60*0.35 + trend=60*0.20 = 27+21+12 = 60
        assert_eq!(calculate_strategic_score("Medium", "Medium", "Rising"), 60);
    }

    #[test]
    fn strategic_score_high_medium_rising() {
        // hook=100*0.45 + share=60*0.35 + trend=60*0.20 = 45+21+12 = 78
        assert_eq!(calculate_strategic_score("High", "Medium", "Rising"), 78);
    }

    #[test]
    fn strategic_score_high_medium_emerging() {
        // hook=100*0.45 + share=60*0.35 + trend=80*0.20 = 45+21+16 = 82
        assert_eq!(calculate_strategic_score("High", "Medium", "Emerging"), 82);
    }

    #[test]
    fn strategic_score_top10_top25_emerging() {
        // hook=100(Top10%)*0.45 + share=60(Top25%)*0.35 + trend=80*0.20 = 45+21+16 = 82
        assert_eq!(calculate_strategic_score("Top 10%", "Top 25%", "Emerging"), 82);
    }

    #[test]
    fn strategic_score_high_top40_rising() {
        // hook=100*0.45 + share=60(Top40%)*0.35 + trend=60*0.20 = 45+21+12 = 78
        assert_eq!(calculate_strategic_score("High", "Top 40%", "Rising"), 78);
    }

    #[test]
    fn strategic_score_case_insensitive() {
        assert_eq!(
            calculate_strategic_score("HIGH", "HIGH", "PEAKING"),
            calculate_strategic_score("High", "High", "Peaking"),
        );
    }

    #[test]
    fn strategic_score_unknown_inputs_clamp_to_30_each() {
        // all unknown → all 30 → 30
        assert_eq!(calculate_strategic_score("garbage", "garbage", "garbage"), 30);
    }

    #[test]
    fn strategic_score_medium_low_stable() {
        // hook=60*0.45 + share=30*0.35 + trend=30*0.20 = 27+10.5+6 = 43.5 → 44
        assert_eq!(calculate_strategic_score("Medium", "Low", "Stable"), 44);
    }

    #[test]
    fn strategic_score_low_medium_rising() {
        // hook=30*0.45 + share=60*0.35 + trend=60*0.20 = 13.5+21+12 = 46.5 → 47
        assert_eq!(calculate_strategic_score("Low", "Medium", "Rising"), 47);
    }

    #[test]
    fn strategic_score_high_high_emerging() {
        // hook=100*0.45 + share=100*0.35 + trend=80*0.20 = 45+35+16 = 96
        assert_eq!(calculate_strategic_score("High", "High", "Emerging"), 96);
    }
}
