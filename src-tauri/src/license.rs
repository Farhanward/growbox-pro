use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const APP_ID: &str = "app.growbox.pro";
pub const RENEWAL_PHONE: &str = "+966504211844";
const PUBLIC_KEY_HEX: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
const CLOCK_ROLLBACK_GRACE_MINUTES: i64 = 10;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseClaims {
    pub license_id: String,
    pub customer_email_hash: String,
    pub phone_hash: String,
    pub issued_at: String,
    pub expires_at: String,
    pub features: Vec<String>,
    pub app_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseEnvelope {
    pub claims: LicenseClaims,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseStatus {
    pub active: bool,
    pub message: String,
    pub expires_at: Option<String>,
    pub days_remaining: Option<i64>,
    pub renewal_warning: bool,
    pub renewal_phone: String,
    pub features: Vec<String>,
}

fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let dir = base.join("GrowBoxPro");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn license_path() -> PathBuf {
    data_dir().join("license.gbl")
}

fn clock_path() -> PathBuf {
    data_dir().join("clock_guard.txt")
}

pub fn canonical_claims(claims: &LicenseClaims) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(claims)?)
}

fn decode_license(code: &str) -> Result<LicenseEnvelope> {
    let trimmed = code.trim().strip_prefix("GBX-").unwrap_or(code.trim());
    let bytes = URL_SAFE_NO_PAD
        .decode(trimmed)
        .context("صيغة كود الاشتراك غير صحيحة")?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn verify_envelope(env: &LicenseEnvelope) -> Result<()> {
    if env.claims.app_id != APP_ID {
        return Err(anyhow!("هذا الكود غير مخصص لهذا التطبيق"));
    }
    let public_bytes: [u8; 32] = hex::decode(PUBLIC_KEY_HEX)?
        .try_into()
        .map_err(|_| anyhow!("مفتاح الترخيص العام غير صحيح"))?;
    let verifying_key = VerifyingKey::from_bytes(&public_bytes)?;
    let signature_bytes: [u8; 64] = URL_SAFE_NO_PAD
        .decode(&env.signature)?
        .try_into()
        .map_err(|_| anyhow!("توقيع الترخيص غير صحيح"))?;
    let signature = Signature::from_bytes(&signature_bytes);
    verifying_key.verify(&canonical_claims(&env.claims)?, &signature)?;
    Ok(())
}

fn persist_last_seen(now: DateTime<Utc>) -> Result<()> {
    fs::write(clock_path(), now.to_rfc3339())?;
    Ok(())
}

fn check_clock(now: DateTime<Utc>) -> Result<()> {
    let path = clock_path();
    if !path.exists() {
        persist_last_seen(now)?;
        return Ok(());
    }
    let last = fs::read_to_string(&path)?;
    let last_seen = DateTime::parse_from_rfc3339(last.trim())?.with_timezone(&Utc);
    if now + Duration::minutes(CLOCK_ROLLBACK_GRACE_MINUTES) < last_seen {
        return Err(anyhow!(
            "تم اكتشاف تغيير في تاريخ الجهاز. يرجى تصحيح التاريخ أو طلب كود جديد من الشركة."
        ));
    }
    if now > last_seen {
        persist_last_seen(now)?;
    }
    Ok(())
}

pub fn activate_license(code: &str) -> Result<LicenseStatus> {
    let env = decode_license(code)?;
    verify_envelope(&env)?;
    fs::write(license_path(), code.trim())?;
    status()
}

pub fn status() -> Result<LicenseStatus> {
    let path = license_path();
    if !path.exists() {
        return Ok(LicenseStatus {
            active: false,
            message: "أدخل كود الاشتراك لتفعيل مميزات GrowBox Pro.".into(),
            expires_at: None,
            days_remaining: None,
            renewal_warning: false,
            renewal_phone: RENEWAL_PHONE.into(),
            features: vec![],
        });
    }

    let code = fs::read_to_string(path)?;
    let env = decode_license(&code)?;
    verify_envelope(&env)?;

    let now = Utc::now();
    check_clock(now)?;
    let expires_at = DateTime::parse_from_rfc3339(&env.claims.expires_at)?.with_timezone(&Utc);
    let remaining = expires_at.signed_duration_since(now);
    let days_remaining = remaining.num_days().max(0);
    let active = remaining.num_seconds() > 0;
    let renewal_warning = active && remaining <= Duration::days(4);
    let message = if active && renewal_warning {
        format!(
            "اشتراكك ينتهي قريبًا. للحفاظ على مميزات التطبيق، يرجى مراسلة الشركة على {} لتجديد الكود.",
            RENEWAL_PHONE
        )
    } else if active {
        format!("الاشتراك فعال. المتبقي {} يوم.", days_remaining)
    } else {
        format!(
            "انتهت مدة الاشتراك. يرجى مراسلة الشركة على {} لتجديد الكود.",
            RENEWAL_PHONE
        )
    };

    Ok(LicenseStatus {
        active,
        message,
        expires_at: Some(expires_at.to_rfc3339()),
        days_remaining: Some(days_remaining),
        renewal_warning,
        renewal_phone: RENEWAL_PHONE.into(),
        features: env.claims.features,
    })
}

pub fn require_active() -> Result<()> {
    let st = status()?;
    if st.active {
        Ok(())
    } else {
        Err(anyhow!(st.message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn accepts_manager_signed_license_envelope() {
        let seed: [u8; 32] = hex::decode("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60")
            .unwrap()
            .try_into()
            .unwrap();
        let claims = LicenseClaims {
            license_id: "test-license".into(),
            customer_email_hash: "email-hash".into(),
            phone_hash: "phone-hash".into(),
            issued_at: "2026-05-13T00:00:00Z".into(),
            expires_at: "2026-06-12T00:00:00Z".into(),
            features: vec!["post_draft".into(), "oauth_read".into()],
            app_id: APP_ID.into(),
        };
        let sig = SigningKey::from_bytes(&seed).sign(&canonical_claims(&claims).unwrap());
        let env = LicenseEnvelope {
            claims,
            signature: URL_SAFE_NO_PAD.encode(sig.to_bytes()),
        };
        verify_envelope(&env).unwrap();
    }
}
