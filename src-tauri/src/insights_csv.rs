// CSV Import — Meta Business Suite Insights export parser.
//
// Meta Business Suite exports CSV with these typical columns (Instagram):
//   Date, Reach, Impressions, Profile Visits, Website Clicks,
//   New Followers, Unfollows, Posts Reach, Posts Saves, Posts Shares
// Audience demographics ship as separate exports with columns like:
//   Age, Gender, Country, City, Percentage
// Online followers export:
//   Hour, Day of Week, Followers Online
//
// We accept any CSV in these shapes and detect which one it is by header.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct InsightsImport {
    pub daily: Vec<DailyMetric>,
    pub demographics: Demographics,
    pub online_hours: Vec<HourlyOnline>,
    pub top_posts: Vec<PostMetric>,
    pub source_file: String,
    pub imported_at: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct DailyMetric {
    pub date: String,
    pub reach: Option<u64>,
    pub impressions: Option<u64>,
    pub profile_visits: Option<u64>,
    pub website_clicks: Option<u64>,
    pub new_followers: Option<u64>,
    pub unfollows: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Demographics {
    pub age_buckets: HashMap<String, f64>,    // "25-34" => 0.42
    pub gender: HashMap<String, f64>,         // "F" => 0.71
    pub top_countries: Vec<(String, f64)>,    // ("SA", 0.42)
    pub top_cities: Vec<(String, f64)>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct HourlyOnline {
    pub day_of_week: String,
    pub hour: u8,
    pub followers_online: u64,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PostMetric {
    pub post_id: String,
    pub caption_preview: Option<String>,
    pub reach: Option<u64>,
    pub saves: Option<u64>,
    pub shares: Option<u64>,
    pub likes: Option<u64>,
    pub comments: Option<u64>,
}

/// Parse a CSV file (or any of the related exports) and return whatever shape
/// it matches. Caller may call multiple times and merge results.
pub fn parse_csv(path: &Path) -> Result<InsightsImport> {
    let text = std::fs::read_to_string(path)?;
    let mut out = InsightsImport {
        source_file: path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("import.csv")
            .to_string(),
        imported_at: chrono::Utc::now().to_rfc3339(),
        ..Default::default()
    };

    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = rdr
        .headers()
        .map_err(|e| anyhow!("csv header read failed: {e}"))?
        .clone();
    let lower: Vec<String> = headers.iter().map(|h| h.to_lowercase()).collect();

    // Detect shape by presence of key columns.
    if lower.iter().any(|h| h.contains("hour")) && lower.iter().any(|h| h.contains("day")) {
        parse_online_hours(&mut rdr, &lower, &mut out)?;
    } else if lower.iter().any(|h| h.contains("age")) || lower.iter().any(|h| h.contains("gender"))
        || lower.iter().any(|h| h.contains("country")) || lower.iter().any(|h| h.contains("city"))
    {
        parse_demographics(&mut rdr, &lower, &mut out)?;
    } else if lower.iter().any(|h| h.contains("saves")) || lower.iter().any(|h| h.contains("post id"))
    {
        parse_posts(&mut rdr, &lower, &mut out)?;
    } else if lower.iter().any(|h| h.contains("date") || h.contains("day")) {
        parse_daily(&mut rdr, &lower, &mut out)?;
    } else {
        return Err(anyhow!(
            "CSV غير معروف الشكل. الأعمدة: {}",
            headers.iter().collect::<Vec<_>>().join(", ")
        ));
    }

    Ok(out)
}

fn idx_of(lower: &[String], needles: &[&str]) -> Option<usize> {
    lower.iter().position(|h| needles.iter().any(|n| h.contains(n)))
}

fn parse_u64(s: &str) -> Option<u64> {
    s.replace(",", "").trim().parse::<u64>().ok()
}
fn parse_f64(s: &str) -> Option<f64> {
    let cleaned = s.replace(",", "").replace("%", "").trim().to_string();
    cleaned.parse::<f64>().ok()
}

fn parse_daily(rdr: &mut csv::Reader<&[u8]>, lower: &[String], out: &mut InsightsImport) -> Result<()> {
    let i_date = idx_of(lower, &["date", "day"]).ok_or_else(|| anyhow!("date col missing"))?;
    let i_reach = idx_of(lower, &["reach"]);
    let i_imp = idx_of(lower, &["impression"]);
    let i_visits = idx_of(lower, &["profile visit", "profile views"]);
    let i_clicks = idx_of(lower, &["website click", "link click"]);
    let i_new = idx_of(lower, &["new follower", "followers gained"]);
    let i_lost = idx_of(lower, &["unfollow", "followers lost"]);

    for r in rdr.records() {
        let row = r?;
        let m = DailyMetric {
            date: row.get(i_date).unwrap_or("").to_string(),
            reach: i_reach.and_then(|i| row.get(i)).and_then(parse_u64),
            impressions: i_imp.and_then(|i| row.get(i)).and_then(parse_u64),
            profile_visits: i_visits.and_then(|i| row.get(i)).and_then(parse_u64),
            website_clicks: i_clicks.and_then(|i| row.get(i)).and_then(parse_u64),
            new_followers: i_new.and_then(|i| row.get(i)).and_then(parse_u64),
            unfollows: i_lost.and_then(|i| row.get(i)).and_then(parse_u64),
        };
        if !m.date.is_empty() {
            out.daily.push(m);
        }
    }
    Ok(())
}

fn parse_demographics(rdr: &mut csv::Reader<&[u8]>, lower: &[String], out: &mut InsightsImport) -> Result<()> {
    let i_label = idx_of(lower, &["age", "gender", "country", "city", "name"])
        .ok_or_else(|| anyhow!("demographics label col missing"))?;
    let i_pct = idx_of(lower, &["percent", "share", "ratio", "%"])
        .or_else(|| idx_of(lower, &["count", "value"]))
        .ok_or_else(|| anyhow!("demographics value col missing"))?;
    let kind = if lower.iter().any(|h| h.contains("age")) {
        "age"
    } else if lower.iter().any(|h| h.contains("gender")) {
        "gender"
    } else if lower.iter().any(|h| h.contains("city")) {
        "city"
    } else if lower.iter().any(|h| h.contains("country")) {
        "country"
    } else {
        "other"
    };

    let mut totals: Vec<(String, f64)> = vec![];
    for r in rdr.records() {
        let row = r?;
        let label = row.get(i_label).unwrap_or("").trim().to_string();
        let pct = row.get(i_pct).and_then(parse_f64).unwrap_or(0.0);
        if !label.is_empty() {
            totals.push((label, pct));
        }
    }
    let sum: f64 = totals.iter().map(|(_, v)| v).sum();
    let normalize = |v: f64| if sum > 0.0 { v / sum } else { 0.0 };

    match kind {
        "age" => for (k, v) in totals { out.demographics.age_buckets.insert(k, normalize(v)); },
        "gender" => for (k, v) in totals { out.demographics.gender.insert(k, normalize(v)); },
        "country" => {
            let mut sorted = totals.into_iter().map(|(k, v)| (k, normalize(v))).collect::<Vec<_>>();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            out.demographics.top_countries = sorted.into_iter().take(10).collect();
        }
        "city" => {
            let mut sorted = totals.into_iter().map(|(k, v)| (k, normalize(v))).collect::<Vec<_>>();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            out.demographics.top_cities = sorted.into_iter().take(10).collect();
        }
        _ => {}
    }
    Ok(())
}

fn parse_online_hours(rdr: &mut csv::Reader<&[u8]>, lower: &[String], out: &mut InsightsImport) -> Result<()> {
    let i_day = idx_of(lower, &["day"]).ok_or_else(|| anyhow!("day col missing"))?;
    let i_hour = idx_of(lower, &["hour"]).ok_or_else(|| anyhow!("hour col missing"))?;
    let i_count = idx_of(lower, &["online", "followers", "count"])
        .ok_or_else(|| anyhow!("followers online col missing"))?;
    for r in rdr.records() {
        let row = r?;
        let day = row.get(i_day).unwrap_or("").trim().to_string();
        let hour = row.get(i_hour).and_then(|s| s.trim().parse::<u8>().ok()).unwrap_or(0);
        let cnt = row.get(i_count).and_then(parse_u64).unwrap_or(0);
        if !day.is_empty() {
            out.online_hours.push(HourlyOnline { day_of_week: day, hour, followers_online: cnt });
        }
    }
    Ok(())
}

fn parse_posts(rdr: &mut csv::Reader<&[u8]>, lower: &[String], out: &mut InsightsImport) -> Result<()> {
    let i_id = idx_of(lower, &["post id", "id"]);
    let i_caption = idx_of(lower, &["caption", "description"]);
    let i_reach = idx_of(lower, &["reach"]);
    let i_saves = idx_of(lower, &["save"]);
    let i_shares = idx_of(lower, &["share"]);
    let i_likes = idx_of(lower, &["like"]);
    let i_comments = idx_of(lower, &["comment"]);
    for r in rdr.records() {
        let row = r?;
        let m = PostMetric {
            post_id: i_id.and_then(|i| row.get(i)).unwrap_or("").to_string(),
            caption_preview: i_caption.and_then(|i| row.get(i)).map(|s| s.chars().take(120).collect()),
            reach: i_reach.and_then(|i| row.get(i)).and_then(parse_u64),
            saves: i_saves.and_then(|i| row.get(i)).and_then(parse_u64),
            shares: i_shares.and_then(|i| row.get(i)).and_then(parse_u64),
            likes: i_likes.and_then(|i| row.get(i)).and_then(parse_u64),
            comments: i_comments.and_then(|i| row.get(i)).and_then(parse_u64),
        };
        if !m.post_id.is_empty() || m.caption_preview.is_some() {
            out.top_posts.push(m);
        }
    }
    // Sort posts by saves desc (algorithm's most-rewarded signal).
    out.top_posts.sort_by(|a, b| b.saves.unwrap_or(0).cmp(&a.saves.unwrap_or(0)));
    Ok(())
}

/// Merge two imports (e.g. user uploads multiple CSV files in one client).
pub fn merge(into: &mut InsightsImport, mut other: InsightsImport) {
    into.daily.append(&mut other.daily);
    into.top_posts.append(&mut other.top_posts);
    into.online_hours.append(&mut other.online_hours);
    if !other.demographics.age_buckets.is_empty() {
        into.demographics.age_buckets = other.demographics.age_buckets;
    }
    if !other.demographics.gender.is_empty() {
        into.demographics.gender = other.demographics.gender;
    }
    if !other.demographics.top_countries.is_empty() {
        into.demographics.top_countries = other.demographics.top_countries;
    }
    if !other.demographics.top_cities.is_empty() {
        into.demographics.top_cities = other.demographics.top_cities;
    }
}

/// Render summary into a context block the LLM can consume directly.
pub fn to_context_block(imp: &InsightsImport) -> String {
    let mut s = String::new();
    s.push_str("## بيانات Insights الخاصة بالحساب (من Meta Business Suite)\n");

    if !imp.daily.is_empty() {
        let total_reach: u64 = imp.daily.iter().filter_map(|d| d.reach).sum();
        let total_imp: u64 = imp.daily.iter().filter_map(|d| d.impressions).sum();
        let total_visits: u64 = imp.daily.iter().filter_map(|d| d.profile_visits).sum();
        let total_clicks: u64 = imp.daily.iter().filter_map(|d| d.website_clicks).sum();
        let new_total: u64 = imp.daily.iter().filter_map(|d| d.new_followers).sum();
        let lost_total: u64 = imp.daily.iter().filter_map(|d| d.unfollows).sum();
        s.push_str(&format!(
            "- مدة البيانات: {} يوم\n- Reach إجمالي: {}\n- Impressions: {}\n- Profile Visits: {}\n- Link Clicks: {}\n- متابعون جدد: {} | فقدان: {} | صافي: {}\n\n",
            imp.daily.len(), total_reach, total_imp, total_visits, total_clicks,
            new_total, lost_total, new_total.saturating_sub(lost_total)
        ));
    }

    if !imp.top_posts.is_empty() {
        s.push_str("### أعلى منشورات حفظاً (Saves = أقوى إشارة للخوارزمية)\n");
        for p in imp.top_posts.iter().take(5) {
            s.push_str(&format!(
                "- Saves: {} · Reach: {} · {}\n",
                p.saves.unwrap_or(0),
                p.reach.unwrap_or(0),
                p.caption_preview.clone().unwrap_or_else(|| "(بدون كابشن)".into())
            ));
        }
        s.push('\n');
    }

    if !imp.demographics.gender.is_empty() || !imp.demographics.age_buckets.is_empty() {
        s.push_str("### Demographics\n");
        for (k, v) in &imp.demographics.gender {
            s.push_str(&format!("- جنس {}: {:.0}%\n", k, v * 100.0));
        }
        let mut ages: Vec<_> = imp.demographics.age_buckets.iter().collect();
        ages.sort_by(|a, b| a.0.cmp(b.0));
        for (k, v) in ages {
            s.push_str(&format!("- عمر {}: {:.0}%\n", k, v * 100.0));
        }
        for (k, v) in imp.demographics.top_cities.iter().take(5) {
            s.push_str(&format!("- مدينة {}: {:.0}%\n", k, v * 100.0));
        }
        s.push('\n');
    }

    if !imp.online_hours.is_empty() {
        // Find top 3 (day, hour) by followers_online.
        let mut sorted = imp.online_hours.clone();
        sorted.sort_by(|a, b| b.followers_online.cmp(&a.followers_online));
        s.push_str("### أعلى ٣ ساعات نشاط للجمهور (من بيانات Insights الفعلية)\n");
        for h in sorted.iter().take(3) {
            s.push_str(&format!("- {} الساعة {}: {} متابع نشط\n", h.day_of_week, h.hour, h.followers_online));
        }
        s.push('\n');
    }

    s
}
