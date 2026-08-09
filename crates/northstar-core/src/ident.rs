//! Shared validation for the filename-safe string segments used by
//! [`crate::package_id::PackageId`], [`crate::puid::AssetPuid`], and
//! [`crate::category::AssetCategory`].
//!
//! All three are validated identically: non-empty, ASCII alphanumeric plus
//! `_` and `-`. Dots are rejected because the `.nspkg` filename grammar
//! (see [`crate::filename`]) reserves `.` to separate an asset PUID from its
//! category — a segment containing a dot would make classification
//! ambiguous without opening the file.

use thiserror::Error;

/// A filename-safe segment failed validation.
///
/// This type is intentionally not part of the public API: each identity
/// newtype wraps it in its own named error so call sites see e.g.
/// `PackageIdError`, not a generic shared error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum IdentError {
    #[error("must not be empty")]
    Empty,
    #[error(
        "contains invalid character {0:?} (only ASCII letters, digits, '_' and '-' are allowed)"
    )]
    InvalidChar(char),
}

pub(crate) fn validate_segment(raw: &str) -> Result<(), IdentError> {
    if raw.is_empty() {
        return Err(IdentError::Empty);
    }
    match raw
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '-'))
    {
        Some(c) => Err(IdentError::InvalidChar(c)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert_eq!(validate_segment(""), Err(IdentError::Empty));
    }

    #[test]
    fn rejects_dot() {
        assert_eq!(validate_segment("a.b"), Err(IdentError::InvalidChar('.')));
    }

    #[test]
    fn accepts_alnum_underscore_hyphen() {
        assert!(validate_segment("pebble_sea-islands2").is_ok());
    }
}
