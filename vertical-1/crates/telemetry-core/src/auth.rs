//! HMAC webhook authentication with constant-time comparison.
//!
//! Spec §3.1:
//! - GitHub: `X-Hub-Signature-256` (sha256=hex)
//! - GitLab: `X-Gitlab-Token` (shared secret)
//! - Jira: `X-Hub-Signature` or shared secret header
//! - Slack: `X-Slack-Signature` + `X-Slack-Request-Timestamp` (v0=HMAC-SHA256)
//! - Linear / Teams / Zendesk: shared secret or signature headers

use crate::error::{CoreError, CoreResult};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use telemetry_proto::SourceProvider;

type HmacSha256 = Hmac<Sha256>;

/// Verify a provider webhook signature / shared secret.
pub fn verify_webhook(
    provider: SourceProvider,
    secret: &str,
    body: &[u8],
    headers: &WebhookHeaders<'_>,
) -> CoreResult<()> {
    match provider {
        SourceProvider::Github => verify_github(secret, body, headers.signature_256),
        SourceProvider::Gitlab => verify_shared_token(secret, headers.gitlab_token),
        SourceProvider::Jira => {
            if let Some(sig) = headers.signature.or(headers.signature_256) {
                verify_github_style(secret, body, sig)
            } else {
                verify_shared_token(secret, headers.shared_secret)
            }
        }
        SourceProvider::Linear => verify_shared_token(secret, headers.shared_secret.or(headers.linear_signature)),
        SourceProvider::Slack => verify_slack(secret, body, headers.slack_signature, headers.slack_timestamp),
        SourceProvider::Teams => verify_shared_token(secret, headers.shared_secret),
        SourceProvider::Zendesk => verify_shared_token(secret, headers.shared_secret),
        SourceProvider::Unspecified => Err(CoreError::Auth("unspecified provider".into())),
    }
}

#[derive(Debug, Clone, Default)]
pub struct WebhookHeaders<'a> {
    pub signature_256: Option<&'a str>,
    pub signature: Option<&'a str>,
    pub gitlab_token: Option<&'a str>,
    pub shared_secret: Option<&'a str>,
    pub linear_signature: Option<&'a str>,
    pub slack_signature: Option<&'a str>,
    pub slack_timestamp: Option<&'a str>,
    pub delivery_id: Option<&'a str>,
    pub event_name: Option<&'a str>,
}

fn verify_github(secret: &str, body: &[u8], signature: Option<&str>) -> CoreResult<()> {
    let sig = signature.ok_or_else(|| CoreError::Auth("missing X-Hub-Signature-256".into()))?;
    verify_github_style(secret, body, sig)
}

fn verify_github_style(secret: &str, body: &[u8], signature: &str) -> CoreResult<()> {
    let hex_part = signature
        .strip_prefix("sha256=")
        .or_else(|| signature.strip_prefix("sha1="))
        .unwrap_or(signature);

    let expected = if signature.starts_with("sha1=") {
        // Legacy Jira-style sha1 — still constant-time compare hex of HMAC-SHA256 of body
        // for simplicity we only support sha256 in production path; accept sha1 prefix
        // by computing sha256 and rejecting (callers should use sha256).
        return Err(CoreError::Auth(
            "sha1 signatures are not accepted; use sha256".into(),
        ));
    } else {
        hmac_sha256_hex(secret.as_bytes(), body)
    };

    if !constant_time_hex_eq(&expected, hex_part) {
        return Err(CoreError::Auth("invalid HMAC signature".into()));
    }
    Ok(())
}

fn verify_shared_token(secret: &str, provided: Option<&str>) -> CoreResult<()> {
    let provided = provided.ok_or_else(|| CoreError::Auth("missing shared secret header".into()))?;
    if !bool::from(secret.as_bytes().ct_eq(provided.as_bytes())) {
        return Err(CoreError::Auth("invalid shared secret".into()));
    }
    Ok(())
}

fn verify_slack(
    secret: &str,
    body: &[u8],
    signature: Option<&str>,
    timestamp: Option<&str>,
) -> CoreResult<()> {
    let signature = signature.ok_or_else(|| CoreError::Auth("missing X-Slack-Signature".into()))?;
    let timestamp =
        timestamp.ok_or_else(|| CoreError::Auth("missing X-Slack-Request-Timestamp".into()))?;

    // Reject stale requests (>5 minutes) to prevent replay.
    if let Ok(ts) = timestamp.parse::<i64>() {
        let now = chrono::Utc::now().timestamp();
        if (now - ts).abs() > 60 * 5 {
            return Err(CoreError::Auth("slack timestamp too old".into()));
        }
    }

    let base = format!("v0:{}:{}", timestamp, String::from_utf8_lossy(body));
    let expected = format!("v0={}", hmac_sha256_hex(secret.as_bytes(), base.as_bytes()));

    if !bool::from(expected.as_bytes().ct_eq(signature.as_bytes())) {
        return Err(CoreError::Auth("invalid slack signature".into()));
    }
    Ok(())
}

pub fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(message);
    let result = mac.finalize().into_bytes();
    hex::encode(result)
}

/// Sign a body the way GitHub does (for synthetic test clients).
pub fn sign_github(secret: &str, body: &[u8]) -> String {
    format!("sha256={}", hmac_sha256_hex(secret.as_bytes(), body))
}

/// Sign a Slack-style request.
pub fn sign_slack(secret: &str, timestamp: &str, body: &[u8]) -> String {
    let base = format!("v0:{}:{}", timestamp, String::from_utf8_lossy(body));
    format!("v0={}", hmac_sha256_hex(secret.as_bytes(), base.as_bytes()))
}

fn constant_time_hex_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_hmac_roundtrip() {
        let secret = "whsec_test";
        let body = br#"{"action":"opened"}"#;
        let sig = sign_github(secret, body);
        let headers = WebhookHeaders {
            signature_256: Some(&sig),
            ..Default::default()
        };
        verify_webhook(SourceProvider::Github, secret, body, &headers).unwrap();
    }

    #[test]
    fn rejects_bad_signature() {
        let body = br#"{"a":1}"#;
        let headers = WebhookHeaders {
            signature_256: Some("sha256=deadbeef"),
            ..Default::default()
        };
        assert!(verify_webhook(SourceProvider::Github, "secret", body, &headers).is_err());
    }

    #[test]
    fn slack_signature_ok() {
        let secret = "slack_secret";
        let body = br#"{"type":"event_callback"}"#;
        let ts = chrono::Utc::now().timestamp().to_string();
        let sig = sign_slack(secret, &ts, body);
        let headers = WebhookHeaders {
            slack_signature: Some(&sig),
            slack_timestamp: Some(&ts),
            ..Default::default()
        };
        verify_webhook(SourceProvider::Slack, secret, body, &headers).unwrap();
    }
}
