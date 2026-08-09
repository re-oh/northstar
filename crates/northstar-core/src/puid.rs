use std::fmt;
use std::str::FromStr;

use thiserror::Error;

use crate::ident::{IdentError, validate_segment};

/// An asset's package-local persistent unique identifier.
///
/// `AssetPuid` is unique *within its owning package only*. Northstar does
/// not require globally coordinated string identifiers for every asset
/// created by every mod author — two different packages may legally contain
/// assets with the same PUID, and those assets remain distinct because
/// canonical identity is the package-qualified pair; see [`crate::AssetKey`].
/// Never use a bare `AssetPuid` as a cross-package key.
///
/// Validated the same way as [`crate::PackageId`]: non-empty, ASCII
/// alphanumeric plus `_` and `-`, no `.`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetPuid(String);

impl AssetPuid {
    pub fn new(raw: impl Into<String>) -> Result<Self, AssetPuidError> {
        let raw = raw.into();
        validate_segment(&raw).map_err(AssetPuidError)?;
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetPuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AssetPuid {
    type Err = AssetPuidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for AssetPuid {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// An [`AssetPuid`] failed validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid asset puid: {0}")]
pub struct AssetPuidError(#[source] IdentError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_round_trips() {
        let puid = AssetPuid::new("pebble_sea_islands").unwrap();
        assert_eq!(puid.to_string(), "pebble_sea_islands");
    }

    #[test]
    fn rejects_dotted_names() {
        assert!(AssetPuid::new("a.b").is_err());
    }
}
