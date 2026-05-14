// Data Fetching Module — spawns playwright-fetcher.mjs against the already-running
// isolated browser (CDP on port 9222/9223) and returns normalized post insights.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

// ── public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct NormalizedInsights {
    pub reach: Option<u64>,
    pub impressions: Option<u64>,
    pub views: Option<u64>,
    pub likes: Option<u64>,
    pub comments: Option<u64>,
    pub shares: Option<u64>,
    pub saves: Option<u64>,
    pub profile_visits: Option<u64>,
    pub engagement_rate: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CapturedEndpoint {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CapturedInsights {
    pub platform: String,
    pub post_url: String,
    pub captured_at: String,
    pub raw_endpoints: Vec<CapturedEndpoint>,
    pub normalized: NormalizedInsights,
    #[serde(default)]
    pub discovered_urls: Vec<String>,
}

// ── context block for LLM injection ─────────────────────────────────────────

pub fn to_context_block(ins: &CapturedInsights) -> String {
    let n = &ins.normalized;
    if !has_real_metrics(ins) {
        return format!(
            "\n## إحصائيات منشور الحساب\n- لا توجد أرقام منشور فعلية من حساب المستخدم حالياً.\n- السبب العملي: الحساب تجريبي أو لا يحتوي منشورات منشورة بعد، أو لم يتم تزويد رابط منشور مباشر.\n- رابط الإدخال: {}\n- التعليمات: لا تخترع أرقاماً للحساب. اعتمد على وصف Vision وأدلة المنافسين والنيتش المحلي فقط، واذكر أن baseline الحساب سيبدأ بعد أول منشور.\n",
            ins.post_url
        );
    }
    let mut s = format!(
        "\n## إحصائيات المنشور الفعلية ({}) — مسحوبة من الجلسة الرسمية\n",
        ins.platform
    );
    s.push_str(&format!("- رابط المنشور: {}\n", ins.post_url));
    if let Some(v) = n.reach.or(n.views) {
        s.push_str(&format!("- الوصول/المشاهدات: {}\n", fmt_num(v)));
    }
    if let Some(v) = n.impressions {
        s.push_str(&format!("- الانطباعات: {}\n", fmt_num(v)));
    }
    if let Some(v) = n.likes {
        s.push_str(&format!("- الإعجابات: {}\n", fmt_num(v)));
    }
    if let Some(v) = n.comments {
        s.push_str(&format!("- التعليقات: {}\n", fmt_num(v)));
    }
    if let Some(v) = n.shares {
        s.push_str(&format!("- المشاركات: {}\n", fmt_num(v)));
    }
    if let Some(v) = n.saves {
        s.push_str(&format!("- الحفظ: {}\n", fmt_num(v)));
    }
    if let Some(v) = n.profile_visits {
        s.push_str(&format!("- زيارات البروفايل من المنشور: {}\n", fmt_num(v)));
    }
    if let Some(v) = n.engagement_rate {
        s.push_str(&format!("- معدل التفاعل الفعلي: {:.2}%\n", v));
    }
    s.push_str(&format!(
        "- عدد نقاط API التي تم اعتراضها: {}\n",
        ins.raw_endpoints.len()
    ));
    s
}

pub fn competitor_context_block(insights: &[CapturedInsights]) -> String {
    if insights.is_empty() {
        return "\n## أدلة المنافسين الفعلية\n- لم يتم جلب منشورات منافسة بأرقام فعلية.\n".into();
    }

    let mut ranked = insights.to_vec();
    ranked.sort_by(|a, b| {
        engagement_score(b)
            .partial_cmp(&engagement_score(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut s = "\n## أفضل 5 منشورات منافسة بأرقام فعلية\n".to_string();
    for (index, item) in ranked.iter().take(5).enumerate() {
        let n = &item.normalized;
        s.push_str(&format!("### مرجع منافس {}\n", index + 1));
        s.push_str(&format!("- الرابط: {}\n", item.post_url));
        if let Some(v) = n.reach.or(n.views) {
            s.push_str(&format!("- الوصول/المشاهدات: {}\n", fmt_num(v)));
        }
        if let Some(v) = n.likes {
            s.push_str(&format!("- الإعجابات: {}\n", fmt_num(v)));
        }
        if let Some(v) = n.comments {
            s.push_str(&format!("- التعليقات: {}\n", fmt_num(v)));
        }
        if let Some(v) = n.shares {
            s.push_str(&format!("- المشاركات: {}\n", fmt_num(v)));
        }
        if let Some(v) = n.saves {
            s.push_str(&format!("- الحفظ: {}\n", fmt_num(v)));
        }
        if let Some(v) = n.engagement_rate {
            s.push_str(&format!("- معدل التفاعل: {:.2}%\n", v));
        }
        s.push_str(&format!("- نقاط API المحتجزة: {}\n", item.raw_endpoints.len()));
    }
    s.push_str("استخدم هذه الأرقام لمعايرة قوة الخطاف، قابلية المشاركة، والهاشتاقات. لا تدّعِ أن منشوراً منافساً قوي إذا كانت أرقامه ناقصة.\n");
    s
}

pub fn engagement_score(ins: &CapturedInsights) -> f64 {
    let n = &ins.normalized;
    let base = n.reach.or(n.views).unwrap_or(0) as f64;
    let engagement = n.likes.unwrap_or(0) + n.comments.unwrap_or(0) + n.shares.unwrap_or(0) + n.saves.unwrap_or(0);
    let rate = n.engagement_rate.unwrap_or_else(|| {
        if base > 0.0 {
            (engagement as f64 / base) * 100.0
        } else {
            0.0
        }
    });
    (rate * 10.0) + base.log10().max(0.0)
}

pub fn has_real_metrics(ins: &CapturedInsights) -> bool {
    let n = &ins.normalized;
    n.reach.is_some()
        || n.impressions.is_some()
        || n.views.is_some()
        || n.likes.is_some()
        || n.comments.is_some()
        || n.shares.is_some()
        || n.saves.is_some()
        || n.profile_visits.is_some()
        || n.engagement_rate.is_some()
}

fn fmt_num(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

// ── fetch entry point ────────────────────────────────────────────────────────

pub async fn fetch(app: &AppHandle, platform: &str, post_url: &str) -> Result<CapturedInsights> {
    if looks_like_profile_url(platform, post_url) {
        return Err(anyhow!(
            "هذا رابط حساب وليس رابط منشور. الحسابات التجريبية التي لا تحتوي منشورات لا تملك أرقام منشور فعلية بعد؛ اترك خانة منشور حسابك فارغة وشغّل التحليل اعتماداً على Vision + المنافسين، أو ضع رابط منشور مباشر مثل /video/ أو /reel/."
        ));
    }

    let script = script_path(app)?;
    if !script.exists() {
        return Err(anyhow!(
            "playwright-fetcher.mjs غير موجود في المسار {}.\n\
             تأكد من بناء التطبيق بشكل صحيح.",
            script.display()
        ));
    }

    let debug_port = debug_port_for(platform);
    ensure_cdp_ready(debug_port).await?;

    let node = node_path(app)?;
    let mut cmd = Command::new(&node);
    let resource_root = script
        .parent()
        .ok_or_else(|| anyhow!("تعذر تحديد مجلد موارد جلب البيانات"))?;
    cmd.arg(&script)
        .arg(platform)
        .arg(post_url)
        .arg(debug_port.to_string())
        .current_dir(resource_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    configure_no_window(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| {
        anyhow!(
            "فشل تشغيل محرك جلب البيانات المحلي: {e}.\n\
             أعد تثبيت GrowBox Pro إذا كانت حزمة التشغيل غير مكتملة."
        )
    })?;

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let app_clone = app.clone();
    let stdout_task = tokio::spawn(async move { read_stdout(stdout, app_clone).await });
    let stderr_task = tokio::spawn(async move { read_stderr(stderr).await });

    let wait_result = tokio::time::timeout(std::time::Duration::from_secs(95), child.wait()).await;
    if wait_result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    let result = stdout_task
        .await
        .context("تعذر قراءة مخرجات جلب البيانات")??;
    let error_msg = stderr_task
        .await
        .context("تعذر قراءة تشخيص جلب البيانات")??;

    result.ok_or_else(|| {
        if wait_result.is_err() {
            anyhow!(
                "انتهت مهلة جلب البيانات. افتح الجلسة المعزولة، تأكد من تسجيل الدخول، ثم أعد المحاولة."
            )
        } else if error_msg.is_empty() {
            anyhow!(
                "لم يتم العثور على JSON صالح من جلسة المنصة. تأكد أن الرابط منشور عام/مسموح وأنك مسجل الدخول داخل المتصفح المعزول."
            )
        } else {
            anyhow!("{error_msg}")
        }
    })
}

pub async fn discover_posts(app: &AppHandle, platform: &str, hashtags: &[String]) -> Result<Vec<String>> {
    let query = hashtags
        .iter()
        .take(8)
        .map(|tag| tag.trim_start_matches('#').to_string())
        .collect::<Vec<_>>()
        .join(",");
    if query.is_empty() {
        return Ok(vec![]);
    }
    let result = fetch(app, platform, &format!("discover:{query}")).await?;
    Ok(result.discovered_urls)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn script_path(app: &AppHandle) -> Result<PathBuf> {
    let path = app
        .path()
        .resource_dir()
        .map_err(|_| anyhow!("resource_dir غير متاح"))?
        .join("resources")
        .join("playwright-fetcher.mjs");
    Ok(path)
}

fn debug_port_for(platform: &str) -> u16 {
    // Must match the ports in isolated_browser.rs.
    match platform {
        "tiktok" => 9223,
        _ => 9222,
    }
}

fn node_path(app: &AppHandle) -> Result<PathBuf> {
    let bundled = app
        .path()
        .resource_dir()
        .map_err(|_| anyhow!("resource_dir غير متاح"))?
        .join("resources")
        .join("node")
        .join(if cfg!(windows) { "node.exe" } else { "node" });
    if bundled.exists() {
        return Ok(bundled);
    }
    Ok(PathBuf::from("node"))
}

fn looks_like_profile_url(platform: &str, url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("discover:") {
        return false;
    }
    if platform == "tiktok" {
        return lower.contains("tiktok.com/@") && !lower.contains("/video/");
    }
    if lower.contains("instagram.com") {
        return !lower.contains("/p/")
            && !lower.contains("/reel/")
            && !lower.contains("/tv/");
    }
    false
}

async fn ensure_cdp_ready(port: u16) -> Result<()> {
    let url = format!("http://127.0.0.1:{port}/json/version");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;
    let response = client.get(&url).send().await.map_err(|_| {
        anyhow!(
            "المتصفح المعزول غير مفتوح أو منفذ CDP {port} غير متاح. افتح الجلسة من خطوة بيانات الحساب أولاً، ثم أعد الجلب."
        )
    })?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "منفذ CDP {port} لا يستجيب بشكل صحيح. أغلق الجلسة المعزولة وافتحها من جديد."
        ));
    }
    Ok(())
}

async fn read_stdout<R>(stdout: R, app: AppHandle) -> Result<Option<CapturedInsights>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut stdout_lines = BufReader::new(stdout).lines();
    let mut result: Option<CapturedInsights> = None;
    while let Some(raw) = stdout_lines.next_line().await? {
        let Some(mut v) = parse_json_line(&raw) else {
            continue;
        };
        if v.get("__progress").and_then(|x| x.as_bool()).unwrap_or(false) {
            let stage = v["stage"].as_str().unwrap_or("fetch").to_string();
            let message = v["message"].as_str().unwrap_or("…").to_string();
            let _ = app.emit(
                "fetch:progress",
                serde_json::json!({ "stage": stage, "message": message }),
            );
        } else if v.get("__result").and_then(|x| x.as_bool()).unwrap_or(false) {
            if let Some(obj) = v.as_object_mut() {
                obj.remove("__result");
            }
            result = serde_json::from_value(v).ok();
        }
    }
    Ok(result)
}

async fn read_stderr<R>(stderr: R) -> Result<String>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut stderr_lines = BufReader::new(stderr).lines();
    let mut messages = Vec::<String>::new();
    while let Some(raw) = stderr_lines.next_line().await? {
        let line = raw.trim();
        if line.is_empty() || is_environment_noise(line) {
            continue;
        }
        if let Some(v) = parse_json_line(line) {
            if let Some(msg) = v.get("error").and_then(|x| x.as_str()).filter(|msg| !is_environment_noise(msg)) {
                push_unique(&mut messages, msg);
            }
        } else {
            push_unique(&mut messages, line);
        }
    }
    Ok(messages.into_iter().take(4).collect::<Vec<_>>().join("\n"))
}

fn parse_json_line(line: &str) -> Option<serde_json::Value> {
    let trimmed = line.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Some(value);
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(&trimmed[start..=end]).ok()
}

fn is_environment_noise(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    lower.starts_with("node.js v")
        || lower.starts_with("(node:")
        || lower.contains("experimentalwarning")
        || lower.contains("deprecationwarning")
        || lower.starts_with("warning:")
        || lower.starts_with("npm notice")
}

fn push_unique(messages: &mut Vec<String>, message: &str) {
    let cleaned = message.trim();
    if cleaned.is_empty() || messages.iter().any(|item| item == cleaned) {
        return;
    }
    messages.push(cleaned.to_string());
}

fn configure_no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_insights() -> CapturedInsights {
        CapturedInsights {
            platform: "tiktok".into(),
            post_url: "حساب جديد بلا منشورات".into(),
            captured_at: "2026-05-14T00:00:00Z".into(),
            raw_endpoints: vec![],
            normalized: NormalizedInsights::default(),
            discovered_urls: vec![],
        }
    }

    #[test]
    fn profile_urls_are_not_treated_as_post_urls() {
        assert!(looks_like_profile_url("tiktok", "https://www.tiktok.com/@new_account"));
        assert!(!looks_like_profile_url("tiktok", "https://www.tiktok.com/@new_account/video/1234567890"));
        assert!(looks_like_profile_url("instagram", "https://www.instagram.com/new_account/"));
        assert!(!looks_like_profile_url("instagram", "https://www.instagram.com/reel/ABC123/"));
        assert!(!looks_like_profile_url("instagram", "https://www.instagram.com/p/ABC123/"));
    }

    #[test]
    fn empty_new_account_baseline_has_no_real_metrics() {
        let mut insights = empty_insights();
        assert!(!has_real_metrics(&insights));
        insights.normalized.views = Some(1200);
        assert!(has_real_metrics(&insights));
    }
}
