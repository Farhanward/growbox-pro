// Orchestrates official/local signals only: OAuth account snapshots, the
// bundled niche matrix, and competitor handles for strategy reference.
// DOM/HTML scraping is intentionally absent; post metrics flow through
// data_fetcher.rs, which intercepts authenticated JSON from the isolated
// browser session.

use crate::{oauth, ClientInput};
use anyhow::Result;
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Default, Clone)]
pub struct AccountSnapshot {
    pub platform: String,
    pub handle: Option<String>,
    pub summary: String,
    pub source: String,
}

#[derive(Debug, Serialize, Default)]
pub struct InsightBundle {
    pub client_profiles: Vec<AccountSnapshot>,
    pub competitor_handles: Vec<String>,
    pub trending_hashtags: Vec<String>,
    pub best_times_weekday: Vec<String>,
    pub best_times_weekend: Vec<String>,
}

fn canonical_niche(niche: &str) -> &str {
    match niche.trim() {
        "مصور" | "مصور فوتوغرافي" | "استوديو تصوير" => "تصوير",
        "مطعم" | "كافيه" | "مقهى" | "حلويات" => "طعام",
        "متجر ملابس" | "عبايات" | "أزياء" | "فاشن" => "موضة",
        other => other,
    }
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
    let node = &v[canonical_niche(niche)];
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

    // 1) official connected-account snapshots. No DOM parsing.
    on_stage("client", "قراءة بيانات الحسابات الرسمية المرتبطة…");
    for (platform, handle) in [
        ("instagram", input.instagram.clone()),
        ("tiktok", input.tiktok.clone()),
    ] {
        if let Ok(account) = oauth::collect_account_data(platform).await {
            bundle.client_profiles.push(AccountSnapshot {
                platform: account.platform,
                handle,
                summary: account.summary,
                source: "official_api".into(),
            });
        }
    }
    if let Some(handle) = input.snapchat.as_ref().filter(|s| !s.trim().is_empty()) {
        bundle.client_profiles.push(AccountSnapshot {
            platform: "snapchat".into(),
            handle: Some(handle.clone()),
            summary: "Snapchat handle محفوظ كمدخل من المستخدم؛ لا توجد عملية HTML scraping.".into(),
            source: "user_input".into(),
        });
    }

    // 2) niche dataset
    on_stage("niche", "تحميل بيانات النيتش الخليجي…");
    let nd = load_niche_data(app, &input.niche, &input.countries);
    bundle.trending_hashtags = nd.hashtags;
    bundle.best_times_weekday = nd.weekday;
    bundle.best_times_weekend = nd.weekend;

    // 3) competitor reference handles only; real metrics must come from
    // authenticated JSON interception via data_fetcher.rs.
    on_stage("competitors", "إضافة مقابض المنافسين كمرجع بدون scraping…");
    bundle.competitor_handles = nd.competitor_handles.into_iter().take(5).collect();

    on_stage("report", "توليد التقرير بالذكاء الاصطناعي…");
    Ok(bundle)
}

pub fn build_context_block(b: &InsightBundle, input: &ClientInput) -> String {
    let mut s = String::new();
    s.push_str(&format!("# بيانات العميل\n- الاسم: {}\n- النيتش: {}\n- الدول: {:?}\n- مدة الخطة: {} يوم\n\n",
        input.name, input.niche, input.countries, input.plan_days));

    if !b.client_profiles.is_empty() {
        s.push_str("## لقطة حسابات العميل (مصادر رسمية/محلية)\n");
        for p in &b.client_profiles {
            s.push_str(&format!(
                "- **{}** {} — المصدر: {}\n  {}\n",
                p.platform,
                p.handle
                    .as_deref()
                    .map(|h| format!("@{}", h.trim_start_matches('@')))
                    .unwrap_or_else(|| "(بدون handle)".into()),
                p.source,
                p.summary
            ));
        }
        s.push('\n');
    }

    if !b.competitor_handles.is_empty() {
        s.push_str("## منافسون مرجعيون من مصفوفة النيتش\n");
        s.push_str("- ");
        s.push_str(&b.competitor_handles.join("\n- "));
        s.push_str("\nملاحظة: لا تُستخدم أرقام منافسين إلا إذا تم جلبها عبر data_fetcher.rs أو CSV رسمي.\n");
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
