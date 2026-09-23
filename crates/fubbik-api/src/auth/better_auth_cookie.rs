//! Verification of better-auth's HMAC-signed session cookie value.
//!
//! better-auth (via better-call) signs cookie values as `${rawToken}.${signature}`, where
//! `signature` is standard (padded) base64 of `HMAC-SHA256(rawToken, secret)`. The split is on
//! the LAST `.` in the value, not the first (see `better-call/dist/context.mjs:44`, which uses
//! `lastIndexOf`).

use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Splits a signed cookie value into `(raw_token, signature_b64)` on the LAST `.`.
pub fn split_signed(value: &str) -> Option<(&str, &str)> {
    let idx = value.rfind('.')?; // LAST dot — better-call uses lastIndexOf
    Some((&value[..idx], &value[idx + 1..]))
}

/// Verifies a better-auth signed cookie value against `secret`, returning the raw token on
/// success.
pub fn verify(cookie_value: &str, secret: &str) -> Option<String> {
    let (token, sig_b64) = split_signed(cookie_value)?;
    // STANDARD base64 with padding, not base64url: better-call signs with btoa()
    // and its own verifier requires 44 chars ending in '='.
    let sig = base64::engine::general_purpose::STANDARD
        .decode(sig_b64)
        .ok()?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(token.as_bytes());
    mac.verify_slice(&sig).ok()?; // constant-time
    Some(token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-value-at-least-32-chars-long-000000";
    const TOKEN: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz012345";
    const SIG: &str = "OyhRBnvMlgxzHjKlrRsQqjXwtDV99cAmrGDWxOTAkzU=";

    // A real HMAC-SHA256(TOKEN2, SECRET), standard-base64-encoded (computed honestly via
    // Python's hmac/base64, searching token values until the signature contained both `+`
    // and `/`). SIG above happens to contain neither, so it decodes identically under
    // STANDARD and URL_SAFE - it can't tell the two engines apart. This one can: `+` and
    // `/` are not in the URL_SAFE alphabet (which uses `-` and `_` instead), so switching
    // the engine at line 24 to URL_SAFE makes `.decode()` fail outright on this fixture.
    const TOKEN2: &str = "OhbVrpoiVgRV5IfLBcbfnoGMbJmTPSIA";
    const SIG2: &str = "Pi3J0fsd+AmVxEWC6E0wD4ogSLZbc38P/WBMT/6Fwts=";

    #[test]
    fn accepts_a_cookie_signed_by_better_auth() {
        // Given the inline inputs and test fixtures.
        // When
        let cookie = format!("{TOKEN}.{SIG}");
        // Then
        assert_eq!(verify(&cookie, SECRET).as_deref(), Some(TOKEN));
    }

    #[test]
    fn rejects_a_tampered_signature() {
        // Given the inline inputs and test fixtures.
        // When
        // Flip one character. A verifier tested only on the happy path proves nothing:
        // the previous slice spent three attempts learning that lesson.
        let bad = format!("{TOKEN}.XyhRBnvMlgxzHjKlrRsQqjXwtDV99cAmrGDWxOTAkzU=");
        // Then
        assert_eq!(
            verify(&bad, SECRET),
            None,
            "a forged signature must not authenticate"
        );
    }

    #[test]
    fn rejects_a_tampered_token() {
        // Given the inline inputs and test fixtures.
        // When
        let bad = format!("BbCdEfGhIjKlMnOpQrStUvWxYz012345.{SIG}");
        // Then
        assert_eq!(
            verify(&bad, SECRET),
            None,
            "the signature must cover the token"
        );
    }

    #[test]
    fn rejects_a_cookie_with_no_signature() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_eq!(verify(TOKEN, SECRET), None);
    }

    #[test]
    fn accepts_a_cookie_whose_signature_contains_reserved_base64url_characters() {
        // Given the inline inputs and test fixtures.
        // When
        // Proves STANDARD (not URL_SAFE) is actually load-bearing: SIG contains neither
        // `+` nor `/`, so it can't distinguish the two engines. This fixture can.
        let cookie = format!("{TOKEN2}.{SIG2}");
        // Then
        assert_eq!(verify(&cookie, SECRET).as_deref(), Some(TOKEN2));
    }

    #[test]
    fn splits_on_the_last_dot_not_the_first() {
        // Given the inline inputs and test fixtures.
        // When
        // better-call uses lastIndexOf (context.mjs:44). Tokens are alphanumeric today, so
        // this is unobservable in production — which is exactly why it needs a test: a
        // first-dot split would work until the token alphabet ever changed.
        let cookie = format!("a.b.{SIG}");
        // Then
        // The signature will not match "a.b", but the SPLIT must still yield "a.b".
        assert_eq!(verify(&cookie, SECRET), None);
        assert_eq!(split_signed(&cookie), Some(("a.b", SIG)));
    }
}
