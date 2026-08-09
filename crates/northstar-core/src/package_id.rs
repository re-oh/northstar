use std::fmt;
use std::str::FromStr;

use thiserror::Error;

use crate::ident::{IdentError, validate_segment};

/// The durable identity of a Northstar content package.
///
/// The permanent encoding and issuance policy for package identities has not
/// been decided (see `docs/architecture.md`). This type exists precisely so
/// that decision can be deferred: every call site depends on `PackageId`,
/// never directly on a Steam Workshop item id, a filesystem path, a display
/// name, or any other convenient-but-wrong stand-in. Steam Workshop ids in
/// particular are a *distribution* binding associated with a `PackageId`,
/// not the identity itself.
///
/// For this experimental slice a `PackageId` is validated as a
/// filename-safe segment: non-empty, ASCII alphanumeric plus `_` and `-`,
/// with no `.` (dots are reserved by the `.nspkg` filename grammar to
/// separate an asset PUID from its category — see [`crate::filename`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageId(String);

impl PackageId {
    /// Validate and wrap a raw string as a package identity.
    pub fn new(raw: impl Into<String>) -> Result<Self, PackageIdError> {
        let raw = raw.into();
        validate_segment(&raw).map_err(PackageIdError)?;
        Ok(Self(raw))
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for PackageId {
    type Err = PackageIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for PackageId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A [`PackageId`] failed validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid package id: {0}")]
pub struct PackageIdError(#[source] IdentError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_round_trips_through_display() {
        let id = PackageId::new("basegame").unwrap();
        assert_eq!(id.to_string(), "basegame");
        assert_eq!(id.as_str(), "basegame");
    }

    #[test]
    fn rejects_dotted_names() {
        assert!(PackageId::new("base.game").is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(PackageId::new("").is_err());
    }

    #[test]
    fn distinct_ids_are_not_equal() {
        assert_ne!(PackageId::new("a").unwrap(), PackageId::new("b").unwrap());
    }
}
