use argon2::Argon2;
use argon2::password_hash::{
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
};
use fubbik_core::error::{AppError, AppResult};
use unicode_normalization::UnicodeNormalization;

pub fn hash_password(plain: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::External(format!("password hashing failed: {e}")))
}

/// Returns false for malformed hashes rather than erroring, so a corrupt
/// stored hash reads as a failed login instead of a 500.
pub fn verify_password(plain: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

/// Better Auth 1.x stores credential hashes as `<hex salt>:<hex key>` and
/// feeds the textual salt into scrypt. This verifier exists only for the
/// one-time login bridge; successful callers immediately receive an
/// Argon2id hash in `user.password_hash`.
pub fn verify_better_auth_password(plain: &str, hash: &str) -> bool {
    use subtle::ConstantTimeEq;

    let Some((salt, expected_hex)) = hash.split_once(':') else {
        return false;
    };
    let Some(expected) = decode_hex(expected_hex) else {
        return false;
    };
    if expected.len() != 64 {
        return false;
    }
    let Ok(params) = scrypt::Params::new(14, 16, 1, 64) else {
        return false;
    };
    let normalized: String = plain.nfkc().collect();
    let mut actual = vec![0u8; 64];
    if scrypt::scrypt(normalized.as_bytes(), salt.as_bytes(), &params, &mut actual).is_err() {
        return false;
    }
    bool::from(actual.ct_eq(&expected))
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16)?;
            let low = (pair[1] as char).to_digit(16)?;
            Some(((high << 4) | low) as u8)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_better_auth_password, verify_password};

    #[test]
    fn verifies_correct_password() {
        // Given the inline inputs and test fixtures.
        // When
        let hash = hash_password("correct horse").unwrap();
        // Then
        assert!(verify_password("correct horse", &hash));
    }

    #[test]
    fn rejects_wrong_password() {
        // Given the inline inputs and test fixtures.
        // When
        let hash = hash_password("correct horse").unwrap();
        // Then
        assert!(!verify_password("wrong horse", &hash));
    }

    #[test]
    fn salts_differ_across_hashes() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert_ne!(
            hash_password("same").unwrap(),
            hash_password("same").unwrap()
        );
    }

    #[test]
    fn rejects_malformed_hash_without_panicking() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(!verify_password("anything", "not-a-phc-string"));
    }

    /// Pins the algorithm identifier so a future dependency bump or config
    /// change can't silently downgrade from argon2id to argon2i/argon2d
    /// without a test failure calling it out.
    #[test]
    fn hash_uses_argon2id_variant() {
        // Given the inline inputs and test fixtures.
        // When
        let hash = hash_password("correct horse").unwrap();
        // Then
        assert!(hash.starts_with("$argon2id$"));
    }

    /// Empty-string edge case: must round-trip like any other password
    /// rather than panicking or short-circuiting.
    #[test]
    fn empty_password_round_trips() {
        // Given the inline inputs and test fixtures.
        // When
        let hash = hash_password("").unwrap();
        // Then
        assert!(verify_password("", &hash));
        assert!(!verify_password("not empty", &hash));
    }

    /// Rejecting an empty hash string specifically (distinct from the
    /// generic "not-a-phc-string" case) guards against a caller passing an
    /// uninitialized/empty DB column straight into verification.
    #[test]
    fn rejects_empty_hash_without_panicking() {
        // Given the inline inputs and test fixtures.
        // When the operation is evaluated by the assertion.
        // Then
        assert!(!verify_password("anything", ""));
    }

    #[test]
    fn verifies_better_auth_scrypt_fixture() {
        // Given
        let hash = "00112233445566778899aabbccddeeff:c3b39f3eda79a45635ff935ee89c8c242531c4d6c6b5fe6bc27a369e3e1e16527bc69395cf710c41dcab0029263692fd327e358e9dc6bcdc7367f97f93ca44a0";
        // When the operation is evaluated by the assertion.
        // Then
        assert!(verify_better_auth_password("legacy-password", hash));
        assert!(!verify_better_auth_password("wrong", hash));
    }
}
