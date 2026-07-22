//! Response body secret redaction (optional safety net).

/// Replace any known secret substrings in `body` with `[REDACTED]`.
///
/// Used so response bodies that echo credentials are not returned verbatim
/// to untrusted callers / logs (TC-S04 style).
pub fn redact_secrets(body: &str, secret_values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut out = body.to_string();
    for s in secret_values {
        let s = s.as_ref();
        if s.is_empty() {
            continue;
        }
        if out.contains(s) {
            out = out.replace(s, "[REDACTED]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_values() {
        let body = r#"{"token":"ghp_supersecret","ok":true}"#;
        let out = redact_secrets(body, ["ghp_supersecret"]);
        assert!(!out.contains("ghp_supersecret"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn empty_secrets_noop() {
        let body = "hello";
        assert_eq!(redact_secrets(body, Vec::<&str>::new()), "hello");
    }
}
