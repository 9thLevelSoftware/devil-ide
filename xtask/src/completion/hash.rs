//! Shared completion-record digest syntax.
//!
//! Evidence, candidate, outcome, and identity-receipt validators must agree
//! on what a SHA-256 looks like. Completion records store lowercase hex.

/// Return whether `value` is a 64-character hexadecimal SHA-256.
///
/// Identity receipts and candidate records accept mixed case and compare
/// with `eq_ignore_ascii_case`. Evidence files use
/// [`is_sha256_lowercase`].
pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.is_ascii() && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Return whether `value` is a 64-character lowercase hexadecimal SHA-256.
///
/// Completion evidence records store lowercase hex only.
pub(crate) fn is_sha256_lowercase(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Return whether `value` is a 40-character lowercase hexadecimal git SHA.
pub(crate) fn is_git_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
