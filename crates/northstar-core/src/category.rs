use std::fmt;
use std::str::FromStr;

use thiserror::Error;

use crate::ident::{IdentError, validate_segment};

/// An asset's category label, e.g. `map`, `mission`, `prefab`.
///
/// A category is *classification*, not identity: it says how to decode an
/// asset's bytes, not which asset it is. It is deliberately open-ended —
/// this type does not enumerate known categories, and it must never be
/// turned into a closed enum. Tools must be able to index, copy, inspect,
/// and report an asset with an unrecognized category without misclassifying
/// it as a complete package; only attempting to *load* an unregistered
/// category into a runtime asset type is expected to fail (that failure
/// lives in `northstar-bevy`'s category-handler registry, not here).
///
/// Validated the same way as [`crate::PackageId`] and [`crate::AssetPuid`]:
/// non-empty, ASCII alphanumeric plus `_` and `-`, no `.`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetCategory(String);

impl AssetCategory {
    pub fn new(raw: impl Into<String>) -> Result<Self, AssetCategoryError> {
        let raw = raw.into();
        validate_segment(&raw).map_err(AssetCategoryError)?;
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AssetCategory {
    type Err = AssetCategoryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for AssetCategory {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// An [`AssetCategory`] failed validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid asset category: {0}")]
pub struct AssetCategoryError(#[source] IdentError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_category_is_still_valid() {
        // Category is open-ended: an unrecognized-but-syntactically-valid
        // label must construct successfully.
        assert!(AssetCategory::new("some_future_category").is_ok());
    }

    #[test]
    fn rejects_dotted_names() {
        assert!(AssetCategory::new("a.b").is_err());
    }
}
