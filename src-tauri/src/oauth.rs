use anyhow::{anyhow, Result};
use keyring::Entry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const SERVICE: &str = "GrowBoxProOAuth";
static CALLBACK_RUNNING: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectedAccount {
    pub platform: String,
    pub connected: bool,
    pub display_name: Option<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountData {
    pub platform: String,
    pub summary: String,
    pub raw_json: serde_json::Value,
}

fn entry(platform: &str) -> Result<Entry> {
    Entry::new(SERVICE, platform).map_err(|e| anyhow!(e.to_string()))
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.trim().is_empty())
}

fn pkce_challenge(verifier: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn save_secret(platform: &str, secret: &serde_json::Value) -> Result<()> {
    entry(platform)?.set_password(&secret.to_string())?;
    Ok(())
}

fn read_secret(platform: &str) -> Result<Option<serde_json::Value>> {
    match entry(platform)?.get_password() {
        Ok(s) => Ok(Some(serde_json::from_str(&s)?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow!(e.to_string())),
    }
}

pub fn start_callback_server(app: AppHandle) {
    if CALLBACK_RUNNING.swap(true, Ordering::Relaxed) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let Ok(listener) = TcpListener::bind("127.0.0.1:8766").await else {
            CALLBACK_RUNNING.store(false, Ordering::Relaxed);
            return;
        };
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                continue;
            };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let mut buf = [0u8; 4096];
                let Ok(n) = stream.read(&mut buf).await else {
                    return;
                };
                let req = String::from_utf8_lossy(&buf[..n]);
                let first = req.lines().next().unwrap_or_default();
                let target = first.split_whitespace().nth(1).unwrap_or_default();
                let (platform, code, error) = parse_callback_target(target);

                let body = if code.is_some() {
                    "GrowBox Pro received the authorization code. You can return to the app."
                } else {
                    "GrowBox Pro did not receive an authorization code. Please return to the app and try again."
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes()).await;

                if let Some(platform) = platform {
                    let _ = app.emit(
                        "oauth:callback",
                        serde_json::json!({
                            "platform": platform,
                            "code": code,
                            "error": error,
                        }),
                    );
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            });
        }
    });
}

fn parse_callback_target(target: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut path_and_query = target.splitn(2, '?');
    let path = path_and_query.next().unwrap_or_default();
    let query = path_and_query.next().unwrap_or_default();
    let platform = path
        .strip_prefix("/oauth/")
        .map(|s| s.trim_matches('/').to_string())
        .filter(|s| s == "instagram" || s == "tiktok");
    let mut code = None;
    let mut error = None;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        let decoded = urlencoding::decode(value)
            .map(|v| v.to_string())
            .unwrap_or_else(|_| value.to_string());
        match key {
            "code" => code = Some(decoded),
            "error" | "error_description" => error = Some(decoded),
            _ => {}
        }
    }
    (platform, code, error)
}

pub fn start_oauth(platform: &str) -> Result<String> {
    let verifier = format!("gbx-{}", uuid::Uuid::new_v4().simple());
    save_secret(
        &format!("{}_pkce", platform),
        &serde_json::json!({ "verifier": verifier }),
    )?;

    match platform {
        "tiktok" => {
            let client_key = env("GROWBOX_TIKTOK_CLIENT_KEY")
                .ok_or_else(|| anyhow!("اضبط GROWBOX_TIKTOK_CLIENT_KEY قبل ربط TikTok"))?;
            let redirect_uri = env("GROWBOX_TIKTOK_REDIRECT_URI")
                .unwrap_or_else(|| "http://127.0.0.1:8766/oauth/tiktok".into());
            Ok(format!(
                "https://www.tiktok.com/v2/auth/authorize/?client_key={client_key}&scope=user.info.basic,video.list&response_type=code&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
                urlencoding::encode(&redirect_uri),
                uuid::Uuid::new_v4(),
                pkce_challenge(&verifier)
            ))
        }
        "instagram" => {
            let client_id = env("GROWBOX_INSTAGRAM_CLIENT_ID")
                .ok_or_else(|| anyhow!("اضبط GROWBOX_INSTAGRAM_CLIENT_ID قبل ربط Instagram"))?;
            let redirect_uri = env("GROWBOX_INSTAGRAM_REDIRECT_URI")
                .unwrap_or_else(|| "http://127.0.0.1:8766/oauth/instagram".into());
            Ok(format!(
                "https://www.instagram.com/oauth/authorize?client_id={client_id}&redirect_uri={}&scope=instagram_business_basic,instagram_business_manage_insights&response_type=code&state={}",
                urlencoding::encode(&redirect_uri),
                uuid::Uuid::new_v4()
            ))
        }
        _ => Err(anyhow!("منصة غير مدعومة")),
    }
}

pub async fn complete_oauth(platform: &str, code: &str) -> Result<ConnectedAccount> {
    if code.trim().is_empty() {
        return Err(anyhow!("كود الربط فارغ"));
    }
    match platform {
        "tiktok" => complete_tiktok(code).await,
        "instagram" => complete_instagram(code).await,
        _ => Err(anyhow!("منصة غير مدعومة")),
    }
}

async fn complete_tiktok(code: &str) -> Result<ConnectedAccount> {
    let client_key = env("GROWBOX_TIKTOK_CLIENT_KEY")
        .ok_or_else(|| anyhow!("اضبط GROWBOX_TIKTOK_CLIENT_KEY"))?;
    let client_secret = env("GROWBOX_TIKTOK_CLIENT_SECRET")
        .ok_or_else(|| anyhow!("اضبط GROWBOX_TIKTOK_CLIENT_SECRET"))?;
    let redirect_uri = env("GROWBOX_TIKTOK_REDIRECT_URI")
        .unwrap_or_else(|| "http://127.0.0.1:8766/oauth/tiktok".into());
    let verifier = read_secret("tiktok_pkce")?
        .and_then(|v| v["verifier"].as_str().map(str::to_owned))
        .ok_or_else(|| anyhow!("انتهت جلسة الربط، أعد المحاولة"))?;

    let res: serde_json::Value = reqwest::Client::new()
        .post("https://open.tiktokapis.com/v2/oauth/token/")
        .form(&[
            ("client_key", client_key.as_str()),
            ("client_secret", client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    save_secret("tiktok", &res)?;
    Ok(ConnectedAccount {
        platform: "tiktok".into(),
        connected: true,
        display_name: None,
        scopes: vec!["user.info.basic".into(), "video.list".into()],
    })
}

async fn complete_instagram(code: &str) -> Result<ConnectedAccount> {
    let client_id = env("GROWBOX_INSTAGRAM_CLIENT_ID")
        .ok_or_else(|| anyhow!("اضبط GROWBOX_INSTAGRAM_CLIENT_ID"))?;
    let client_secret = env("GROWBOX_INSTAGRAM_CLIENT_SECRET")
        .ok_or_else(|| anyhow!("اضبط GROWBOX_INSTAGRAM_CLIENT_SECRET"))?;
    let redirect_uri = env("GROWBOX_INSTAGRAM_REDIRECT_URI")
        .unwrap_or_else(|| "http://127.0.0.1:8766/oauth/instagram".into());

    let res: serde_json::Value = reqwest::Client::new()
        .post("https://api.instagram.com/oauth/access_token")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
            ("code", code),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    save_secret("instagram", &res)?;
    Ok(ConnectedAccount {
        platform: "instagram".into(),
        connected: true,
        display_name: None,
        scopes: vec![
            "instagram_business_basic".into(),
            "instagram_business_manage_insights".into(),
        ],
    })
}

pub fn connected_accounts() -> Result<Vec<ConnectedAccount>> {
    Ok(["instagram", "tiktok"]
        .iter()
        .map(|platform| ConnectedAccount {
            platform: (*platform).into(),
            connected: read_secret(platform).ok().flatten().is_some(),
            display_name: None,
            scopes: if *platform == "tiktok" {
                vec!["user.info.basic".into(), "video.list".into()]
            } else {
                vec![
                    "instagram_business_basic".into(),
                    "instagram_business_manage_insights".into(),
                ]
            },
        })
        .collect())
}

pub fn disconnect(platform: &str) -> Result<()> {
    match entry(platform)?.delete_credential() {
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow!(e.to_string())),
    }
}

pub async fn collect_account_data(platform: &str) -> Result<AccountData> {
    let token = read_secret(platform)?.ok_or_else(|| anyhow!("اربط الحساب أولاً"))?;
    match platform {
        "tiktok" => collect_tiktok_data(token).await,
        "instagram" => collect_instagram_data(token).await,
        _ => Err(anyhow!("منصة غير مدعومة")),
    }
}

async fn collect_tiktok_data(token: serde_json::Value) -> Result<AccountData> {
    let access_token = token["access_token"]
        .as_str()
        .ok_or_else(|| anyhow!("TikTok token ناقص"))?;
    let client = reqwest::Client::new();
    let user: serde_json::Value = client
        .get("https://open.tiktokapis.com/v2/user/info/?fields=open_id,union_id,avatar_url,display_name,bio_description,profile_deep_link,is_verified,follower_count,following_count,likes_count,video_count")
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let videos: serde_json::Value = client
        .post("https://open.tiktokapis.com/v2/video/list/?fields=id,title,video_description,duration,cover_image_url,share_url,view_count,like_count,comment_count,share_count")
        .bearer_auth(access_token)
        .json(&serde_json::json!({"max_count": 10}))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .unwrap_or_else(|_| serde_json::json!({"error": "video.list unavailable"}));
    let display = user["data"]["user"]["display_name"].as_str().unwrap_or("TikTok");
    Ok(AccountData {
        platform: "tiktok".into(),
        summary: format!("TikTok: تم جمع بيانات الحساب والفيديوهات الحديثة لـ {display}."),
        raw_json: serde_json::json!({ "user": user, "videos": videos }),
    })
}

async fn collect_instagram_data(token: serde_json::Value) -> Result<AccountData> {
    let access_token = token["access_token"]
        .as_str()
        .ok_or_else(|| anyhow!("Instagram token ناقص"))?;
    let me: serde_json::Value = reqwest::Client::new()
        .get("https://graph.instagram.com/v21.0/me")
        .query(&[
            ("fields", "user_id,username,account_type,media_count"),
            ("access_token", access_token),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let username = me["username"].as_str().unwrap_or("Instagram");
    Ok(AccountData {
        platform: "instagram".into(),
        summary: format!("Instagram: تم جمع بيانات الحساب الأساسية لـ {username}. المؤشرات تعتمد على صلاحيات الحساب الاحترافي المعتمدة من Meta."),
        raw_json: serde_json::json!({ "profile": me }),
    })
}
