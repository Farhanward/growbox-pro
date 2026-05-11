// Public-data scrapers (no login). Stub layer: real selectors plug in later.
// Each function takes a public username and returns a serializable snapshot.
//
// Strategy:
// - Instagram: use the public web profile JSON (`?__a=1` is dead, fall back to og: meta tags).
// - TikTok: public profile HTML — read meta tags + visible follower counts.
// - Snapchat: public story page reachable via https://story.snapchat.com/@<user>.

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Serialize, Default)]
pub struct PublicProfile {
    pub platform: String,
    pub username: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub followers: Option<u64>,
    pub posts: Option<u64>,
}

#[allow(dead_code)]
pub async fn fetch_instagram(username: &str) -> Result<PublicProfile> {
    Ok(PublicProfile {
        platform: "instagram".into(),
        username: username.into(),
        ..Default::default()
    })
}

#[allow(dead_code)]
pub async fn fetch_tiktok(username: &str) -> Result<PublicProfile> {
    Ok(PublicProfile {
        platform: "tiktok".into(),
        username: username.into(),
        ..Default::default()
    })
}

#[allow(dead_code)]
pub async fn fetch_snapchat(username: &str) -> Result<PublicProfile> {
    Ok(PublicProfile {
        platform: "snapchat".into(),
        username: username.into(),
        ..Default::default()
    })
}
