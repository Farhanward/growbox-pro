// Public-data scrapers — NO login, NO bots, NO stealth.
// We just hit the public profile URL with a real-browser User-Agent and
// extract whatever the page exposes to anyone who opens it in Safari.
//
// Both platforms occasionally restructure their HTML; failures here are
// non-fatal — the orchestrator falls back to user-provided inputs.

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 \
                  (KHTML, like Gecko) Version/17.5 Safari/605.1.15";

#[derive(Debug, Serialize, Default, Clone)]
pub struct PublicProfile {
    pub platform: String,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub followers: Option<u64>,
    pub following: Option<u64>,
    pub posts: Option<u64>,
    pub recent_captions: Vec<String>,
    pub recent_hashtags: Vec<String>,
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(UA)
        .timeout(std::time::Duration::from_secs(15))
        .build()?)
}

fn parse_human_number(s: &str) -> Option<u64> {
    let s = s.replace(',', "").replace('\u{066C}', ""); // Arabic thousands sep
    let last = s.chars().last()?;
    let (mult, rest) = match last {
        'K' | 'k' => (1_000.0, &s[..s.len() - 1]),
        'M' | 'm' => (1_000_000.0, &s[..s.len() - 1]),
        'B' | 'b' => (1_000_000_000.0, &s[..s.len() - 1]),
        _ => (1.0, s.as_str()),
    };
    rest.trim().parse::<f64>().ok().map(|n| (n * mult) as u64)
}

// -------- Instagram --------
static IG_OG_TITLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"<meta property="og:title" content="([^"]+)""#).unwrap());
static IG_OG_DESC: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"<meta property="og:description" content="([^"]+)""#).unwrap());
static IG_OG_IMG_ALT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#""biography"\s*:\s*"((?:[^"\\]|\\.)*)""#).unwrap());
static IG_STATS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"([\d.,]+[KMB]?)\s*Followers?,\s*([\d.,]+[KMB]?)\s*Following,\s*([\d.,]+[KMB]?)\s*Posts?"#).unwrap());

pub async fn fetch_instagram(username: &str) -> Result<PublicProfile> {
    let u = username.trim().trim_start_matches('@');
    if u.is_empty() {
        return Err(anyhow!("empty IG username"));
    }
    let url = format!("https://www.instagram.com/{}/", u);
    let html = client()?.get(&url).send().await?.error_for_status()?.text().await?;

    let mut p = PublicProfile {
        platform: "instagram".into(),
        username: u.into(),
        ..Default::default()
    };
    if let Some(c) = IG_OG_TITLE.captures(&html) {
        p.display_name = Some(c[1].to_string());
    }
    if let Some(c) = IG_OG_DESC.captures(&html) {
        if let Some(s) = IG_STATS.captures(c.get(1).unwrap().as_str()) {
            p.followers = parse_human_number(&s[1]);
            p.following = parse_human_number(&s[2]);
            p.posts = parse_human_number(&s[3]);
        }
    }
    if let Some(c) = IG_OG_IMG_ALT.captures(&html) {
        p.bio = Some(c[1].replace("\\n", " ").replace("\\\"", "\""));
    }
    // Hashtags from any caption-like text in the page.
    let tag_re = Regex::new(r"#[؀-ۿa-zA-Z0-9_]+").unwrap();
    let mut tags: Vec<String> = tag_re.find_iter(&html).map(|m| m.as_str().to_string()).collect();
    tags.sort();
    tags.dedup();
    p.recent_hashtags = tags.into_iter().take(20).collect();
    Ok(p)
}

// -------- TikTok --------
static TT_DATA: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<script id="__UNIVERSAL_DATA_FOR_REHYDRATION__"[^>]*>([\s\S]*?)</script>"#).unwrap()
});

pub async fn fetch_tiktok(username: &str) -> Result<PublicProfile> {
    let u = username.trim().trim_start_matches('@');
    if u.is_empty() {
        return Err(anyhow!("empty TT username"));
    }
    let url = format!("https://www.tiktok.com/@{}", u);
    let html = client()?.get(&url).send().await?.error_for_status()?.text().await?;

    let mut p = PublicProfile {
        platform: "tiktok".into(),
        username: u.into(),
        ..Default::default()
    };

    if let Some(c) = TT_DATA.captures(&html) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(c.get(1).unwrap().as_str()) {
            let ui = &v["__DEFAULT_SCOPE__"]["webapp.user-detail"]["userInfo"];
            let user = &ui["user"];
            let stats = &ui["stats"];
            p.display_name = user["nickname"].as_str().map(String::from);
            p.bio = user["signature"].as_str().map(String::from);
            p.followers = stats["followerCount"].as_u64();
            p.following = stats["followingCount"].as_u64();
            p.posts = stats["videoCount"].as_u64();
        }
    }
    let tag_re = Regex::new(r"#[؀-ۿa-zA-Z0-9_]+").unwrap();
    let mut tags: Vec<String> = tag_re.find_iter(&html).map(|m| m.as_str().to_string()).collect();
    tags.sort();
    tags.dedup();
    p.recent_hashtags = tags.into_iter().take(20).collect();
    Ok(p)
}

// -------- Snapchat (very limited public data) --------
pub async fn fetch_snapchat(username: &str) -> Result<PublicProfile> {
    let u = username.trim().trim_start_matches('@');
    if u.is_empty() {
        return Err(anyhow!("empty SC username"));
    }
    let url = format!("https://www.snapchat.com/add/{}", u);
    let html = client()?.get(&url).send().await?.error_for_status()?.text().await?;
    let mut p = PublicProfile {
        platform: "snapchat".into(),
        username: u.into(),
        ..Default::default()
    };
    if let Some(c) = Regex::new(r#"<meta property="og:title" content="([^"]+)""#)
        .unwrap()
        .captures(&html)
    {
        p.display_name = Some(c[1].to_string());
    }
    if let Some(c) = Regex::new(r#"<meta property="og:description" content="([^"]+)""#)
        .unwrap()
        .captures(&html)
    {
        p.bio = Some(c[1].to_string());
    }
    Ok(p)
}
