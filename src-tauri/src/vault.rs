use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use keyring::Entry;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SERVICE: &str = "GrowBoxProSecureVault";
const KEY_NAME: &str = "aes256-master-key";
const VAULT_FILE: &str = "secure-vault.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoginCredentialInput {
    pub platform: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoginCredentialSummary {
    pub id: String,
    pub platform: String,
    pub username: String,
    pub profile_label: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LoginSecret {
    pub platform: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StoredCredential {
    id: String,
    platform: String,
    username: String,
    profile_label: String,
    nonce: String,
    ciphertext: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct VaultFile {
    version: u8,
    credentials: Vec<StoredCredential>,
}

fn app_dir() -> PathBuf {
    let dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("GrowBoxPro");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn vault_path() -> PathBuf {
    app_dir().join(VAULT_FILE)
}

fn key_entry() -> Result<Entry> {
    Entry::new(SERVICE, KEY_NAME).map_err(|e| anyhow!(e.to_string()))
}

fn master_key() -> Result<[u8; 32]> {
    match key_entry()?.get_password() {
        Ok(encoded) => {
            let bytes = STANDARD.decode(encoded)?;
            let key: [u8; 32] = bytes
                .try_into()
                .map_err(|_| anyhow!("مفتاح vault المحلي ليس AES-256 صالحاً"))?;
            Ok(key)
        }
        Err(keyring::Error::NoEntry) => {
            let mut key = [0u8; 32];
            OsRng.fill_bytes(&mut key);
            key_entry()?.set_password(&STANDARD.encode(key))?;
            Ok(key)
        }
        Err(e) => Err(anyhow!(e.to_string())),
    }
}

fn load_vault() -> Result<VaultFile> {
    let path = vault_path();
    if !path.exists() {
        return Ok(VaultFile { version: 1, credentials: vec![] });
    }
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text).unwrap_or_else(|_| VaultFile { version: 1, credentials: vec![] }))
}

fn save_vault(vault: &VaultFile) -> Result<()> {
    let text = serde_json::to_string_pretty(vault)?;
    std::fs::write(vault_path(), text)?;
    Ok(())
}

fn cipher() -> Result<Aes256Gcm> {
    let key = master_key()?;
    Ok(Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow!("فشل تهيئة AES-256"))?)
}

fn encrypt(secret: &LoginSecret) -> Result<(String, String)> {
    let cipher = cipher()?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let plaintext = serde_json::to_vec(secret)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_ref())
        .map_err(|_| anyhow!("فشل تشفير بيانات الدخول"))?;
    Ok((STANDARD.encode(nonce_bytes), STANDARD.encode(ciphertext)))
}

pub fn list_credentials() -> Result<Vec<LoginCredentialSummary>> {
    let vault = load_vault()?;
    Ok(vault.credentials.into_iter().map(summary).collect())
}

pub fn save_credential(input: LoginCredentialInput) -> Result<LoginCredentialSummary> {
    let platform = normalize_platform(&input.platform)?;
    let username = input.username.trim().trim_start_matches('@').to_string();
    if username.is_empty() || input.password.is_empty() {
        return Err(anyhow!("اسم المستخدم وكلمة المرور مطلوبان."));
    }

    let secret = LoginSecret {
        platform: platform.clone(),
        username: username.clone(),
        password: input.password,
    };
    let (nonce, ciphertext) = encrypt(&secret)?;
    let mut vault = load_vault()?;
    let now = chrono::Utc::now().to_rfc3339();
    let profile_label = username.clone();

    if let Some(existing) = vault
        .credentials
        .iter_mut()
        .find(|item| item.platform == platform && item.username.eq_ignore_ascii_case(&username))
    {
        existing.profile_label = profile_label;
        existing.nonce = nonce;
        existing.ciphertext = ciphertext;
        existing.updated_at = now;
        let out = summary(existing.clone());
        save_vault(&vault)?;
        return Ok(out);
    }

    let record = StoredCredential {
        id: uuid::Uuid::new_v4().to_string(),
        platform,
        username,
        profile_label,
        nonce,
        ciphertext,
        created_at: now.clone(),
        updated_at: now,
    };
    let out = summary(record.clone());
    vault.version = 1;
    vault.credentials.push(record);
    save_vault(&vault)?;
    Ok(out)
}

pub fn delete_credential(id: &str) -> Result<()> {
    let mut vault = load_vault()?;
    let before = vault.credentials.len();
    vault.credentials.retain(|item| item.id != id);
    if vault.credentials.len() == before {
        return Err(anyhow!("بيانات الدخول غير موجودة."));
    }
    save_vault(&vault)
}

pub fn profile_label_for(id: &str) -> Result<(String, String)> {
    let vault = load_vault()?;
    let record = vault
        .credentials
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| anyhow!("بيانات الدخول غير موجودة."))?;
    Ok((record.platform.clone(), record.profile_label.clone()))
}

fn summary(record: StoredCredential) -> LoginCredentialSummary {
    LoginCredentialSummary {
        id: record.id,
        platform: record.platform,
        username: record.username,
        profile_label: record.profile_label,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn normalize_platform(platform: &str) -> Result<String> {
    match platform.trim().to_ascii_lowercase().as_str() {
        "instagram" | "ig" => Ok("instagram".into()),
        "tiktok" | "tt" => Ok("tiktok".into()),
        _ => Err(anyhow!("منصة غير مدعومة للحفظ الآمن.")),
    }
}
