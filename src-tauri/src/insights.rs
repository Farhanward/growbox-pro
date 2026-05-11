// Orchestrates: scrape client → scrape competitors → load niche data → build
// the LLM context block. Each stage is non-fatal — partial data still yields
// a useful report.

use crate::scrapers::{fetch_instagram, fetch_snapchat, fetch_tiktok, PublicProfile};
use crate::ClientInput;
use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Default)]
pub struct InsightBundle {
    pub client_profiles: Vec<PublicProfile>,
    pub competitors: Vec<PublicProfile>,
    pub trending_hashtags: Vec<String>,
    pub best_times_weekday: Vec<String>,
    pub best_times_weekend: Vec<String>,
}

fn load_niche_data(app: &AppHandle, niche: &str, countries: &[String]) -> NicheData {
    let mut out = NicheData::default();
    let path: PathBuf = app
        .path()
        .resource_dir()
        .map(|p| p.join("resources/data/gulf-niches.json"))
        .unwrap_or_default();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return out;
    };
    let Ok(v): std::result::Result<serde_json::Value, _> = serde_json::from_str(&text) else {
        return out;
    };
    let node = &v[niche];
    if node.is_null() {
        return out;
    }
    if let Some(arr) = node["trending_hashtags"].as_array() {
        out.hashtags = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
    }
    if let Some(arr) = node["best_post_times_gulf"]["weekday"].as_array() {
        out.weekday = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
    }
    if let Some(arr) = node["best_post_times_gulf"]["weekend"].as_array() {
        out.weekend = arr.iter().filter_map(|x| x.as_str().map(String::from)).collect();
    }
    let comps = &node["competitors"];
    for cc in countries {
        if let Some(arr) = comps[cc].as_array() {
            for it in arr {
                if let Some(s) = it.as_str() {
                    out.competitor_handles.push(s.to_string());
                }
            }
        }
    }
    out
}

#[derive(Default)]
struct NicheData {
    hashtags: Vec<String>,
    weekday: Vec<String>,
    weekend: Vec<String>,
    competitor_handles: Vec<String>,
}

pub async fn gather<F>(
    app: &AppHandle,
    input: &ClientInput,
    on_stage: F,
) -> Result<InsightBundle>
where
    F: Fn(&str, &str) + Send + Sync + 'static,
{
    let mut bundle = InsightBundle::default();

    // 1) scrape client's own public profiles
    on_stage("client", "تحليل حسابات العميل…");
    if let Some(u) = input.tiktok.as_deref().filter(|s| !s.trim().is_empty()) {
        if let Ok(p) = fetch_tiktok(u).await {
            bundle.client_profiles.push(p);
        }
    }
    if let Some(u) = input.instagram.as_deref().filter(|s| !s.trim().is_empty()) {
        if let Ok(p) = fetch_instagram(u).await {
            bundle.client_profiles.push(p);
        }
    }
    if let Some(u) = input.snapchat.as_deref().filter(|s| !s.trim().is_empty()) {
        if let Ok(p) = fetch_snapchat(u).await {
            bundle.client_profiles.push(p);
        }
    }

    // 2) niche dataset
    on_stage("niche", "تحميل بيانات النيتش الخليجي…");
    let nd = load_niche_data(app, &input.niche, &input.countries);
    bundle.trending_hashtags = nd.hashtags;
    bundle.best_times_weekday = nd.weekday;
    bundle.best_times_weekend = nd.weekend;

    // 3) competitor scan (cap to 5 to keep latency reasonable)
    on_stage("competitors", "تحليل المنافسين…");
    for handle in nd.competitor_handles.iter().take(5) {
        // try TikTok first (more uniform HTML), fall back to IG
        if let Ok(p) = fetch_tiktok(handle).await {
            if p.followers.is_some() {
                bundle.competitors.push(p);
                continue;
            }
        }
        if let Ok(p) = fetch_instagram(handle).await {
            bundle.competitors.push(p);
        }
    }

    on_stage("report", "توليد التقرير بالذكاء الاصطناعي…");
    Ok(bundle)
}

pub fn build_context_block(b: &InsightBundle, input: &ClientInput) -> String {
    let mut s = String::new();
    s.push_str(&format!("# بيانات العميل\n- الاسم: {}\n- النيتش: {}\n- الدول: {:?}\n- مدة الخطة: {} يوم\n\n",
        input.name, input.niche, input.countries, input.plan_days));

    if !b.client_profiles.is_empty() {
        s.push_str("## لقطة حسابات العميل (بيانات عامة)\n");
        for p in &b.client_profiles {
            s.push_str(&format!(
                "- **{}** @{}: متابعين={}, متابَع={}, منشورات={}\n  البايو: {}\n",
                p.platform,
                p.username,
                p.followers.unwrap_or(0),
                p.following.unwrap_or(0),
                p.posts.unwrap_or(0),
                p.bio.clone().unwrap_or_default()
            ));
        }
        s.push('\n');
    }

    if !b.competitors.is_empty() {
        s.push_str("## منافسون في نفس النيتش (لقطات عامة)\n");
        for c in &b.competitors {
            s.push_str(&format!(
                "- **{}** @{} — متابعين={}, منشورات={}, البايو: {}\n",
                c.platform,
                c.username,
                c.followers.unwrap_or(0),
                c.posts.unwrap_or(0),
                c.bio.clone().unwrap_or_default()
            ));
        }
        s.push('\n');
    }

    if !b.trending_hashtags.is_empty() {
        s.push_str("## هاشتاقات خليجية رائجة في النيتش\n");
        s.push_str(&b.trending_hashtags.join(", "));
        s.push_str("\n\n");
    }

    if !b.best_times_weekday.is_empty() {
        s.push_str(&format!(
            "## أوقات النشر الذهبية (توقيت الخليج)\n- أيام الأسبوع: {}\n- نهاية الأسبوع: {}\n\n",
            b.best_times_weekday.join("، "),
            b.best_times_weekend.join("، ")
        ));
    }

    s
}
