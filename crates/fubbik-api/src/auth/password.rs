use argon2::Argon2;
use argon2::password_hash::{
    PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng,
};
use fubbik_core::error::{AppError, AppResult};

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

#[cfg(test)]
mod tests {
    use super::{hash_password, verify_password};

    #[test]
    fn verifies_correct_password() {
        let hash = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &hash));
    }

    #[test]
    fn rejects_wrong_password() {
        let hash = hash_password("correct horse").unwrap();
        assert!(!verify_password("wrong horse", &hash));
    }

    #[test]
    fn salts_differ_across_hashes() {
        assert_ne!(
            hash_password("same").unwrap(),
            hash_password("same").unwrap()
        );
    }

    #[test]
    fn rejects_malformed_hash_without_panicking() {
        assert!(!verify_password("anything", "not-a-phc-string"));
    }

    /// Pins the algorithm identifier so a future dependency bump or config
    /// change can't silently downgrade from argon2id to argon2i/argon2d
    /// without a test failure calling it out.
    #[test]
    fn hash_uses_argon2id_variant() {
        let hash = hash_password("correct horse").unwrap();
        assert!(hash.starts_with("$argon2id$"));
    }

    /// Empty-string edge case: must round-trip like any other password
    /// rather than panicking or short-circuiting.
    #[test]
    fn empty_password_round_trips() {
        let hash = hash_password("").unwrap();
        assert!(verify_password("", &hash));
        assert!(!verify_password("not empty", &hash));
    }

    /// Rejecting an empty hash string specifically (distinct from the
    /// generic "not-a-phc-string" case) guards against a caller passing an
    /// uninitialized/empty DB column straight into verification.
    #[test]
    fn rejects_empty_hash_without_panicking() {
        assert!(!verify_password("anything", ""));
    }
}
