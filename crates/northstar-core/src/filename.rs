//! The `.nspkg` filename grammar and its classifier.
//!
//! ```text
//! <package_puid>.nspkg
//! <asset_puid>.<asset_category>.nspkg
//! ```
//!
//! Parsing goes from the right: after removing the `.nspkg` suffix, a final
//! `.` separates an asset PUID from its category. With no remaining `.`, the
//! file is a complete package. This is why [`crate::PackageId`],
//! [`crate::AssetPuid`], and [`crate::AssetCategory`] all reject `.` in
//! their own validation — a segment containing a dot would make this rule
//! ambiguous.
//!
//! Classification never opens or reads the file; it operates on the
//! filename text alone.

use std::path::Path;

use thiserror::Error;

use crate::category::{AssetCategory, AssetCategoryError};
use crate::package_id::{PackageId, PackageIdError};
use crate::puid::{AssetPuid, AssetPuidError};

const EXTENSION: &str = ".nspkg";

/// The two `.nspkg` filename forms, distinguishable without reading the
/// file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifiedFilename {
    /// `<package_puid>.nspkg` — a complete, mountable content package.
    Package { puid: PackageId },
    /// `<asset_puid>.<category>.nspkg` — one individually packaged asset.
    ///
    /// `category` is preserved even when this build of Northstar does not
    /// know how to load it — see [`crate::AssetCategory`].
    Asset {
        puid: AssetPuid,
        category: AssetCategory,
    },
}

impl ClassifiedFilename {
    /// Classify a bare filename (not a full path) using only its text.
    /// Performs no file I/O; the name does not need to correspond to a file
    /// that exists.
    pub fn classify(filename: &str) -> Result<Self, ClassifyError> {
        let stem = filename
            .strip_suffix(EXTENSION)
            .ok_or(ClassifyError::MissingExtension)?;

        match stem.rsplit_once('.') {
            None => {
                let puid = PackageId::new(stem).map_err(ClassifyError::InvalidPackagePuid)?;
                Ok(Self::Package { puid })
            }
            Some((puid, category)) => {
                let puid = AssetPuid::new(puid).map_err(ClassifyError::InvalidAssetPuid)?;
                let category =
                    AssetCategory::new(category).map_err(ClassifyError::InvalidCategory)?;
                Ok(Self::Asset { puid, category })
            }
        }
    }

    /// Classify the filename component of `path`. Still performs no file
    /// I/O — `path` never needs to exist.
    pub fn classify_path(path: &Path) -> Result<Self, ClassifyError> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(ClassifyError::NotUtf8Filename)?;
        Self::classify(filename)
    }

    /// True if this is a complete content package rather than an individual
    /// asset.
    pub fn is_package(&self) -> bool {
        matches!(self, Self::Package { .. })
    }
}

/// A filename could not be classified under the `.nspkg` grammar.
///
/// Malformed names never fall back to guessing or normalizing — they are
/// reported explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClassifyError {
    #[error("filename does not end in \"{EXTENSION}\"")]
    MissingExtension,
    #[error("path has no UTF-8 filename component")]
    NotUtf8Filename,
    #[error("malformed .nspkg filename: {0}")]
    InvalidPackagePuid(#[source] PackageIdError),
    #[error("malformed .nspkg filename: {0}")]
    InvalidAssetPuid(#[source] AssetPuidError),
    #[error("malformed .nspkg filename: {0}")]
    InvalidCategory(#[source] AssetCategoryError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basegame_is_a_complete_package() {
        let c = ClassifiedFilename::classify("basegame.nspkg").unwrap();
        assert!(c.is_package());
        assert_eq!(
            c,
            ClassifiedFilename::Package {
                puid: PackageId::new("basegame").unwrap()
            }
        );
    }

    #[test]
    fn map_asset_classifies_with_puid_and_category() {
        let c = ClassifiedFilename::classify("pebble_sea_islands.map.nspkg").unwrap();
        assert_eq!(
            c,
            ClassifiedFilename::Asset {
                puid: AssetPuid::new("pebble_sea_islands").unwrap(),
                category: AssetCategory::new("map").unwrap(),
            }
        );
    }

    #[test]
    fn mission_asset_classifies_with_puid_and_category() {
        let c = ClassifiedFilename::classify("oil_rig_protection.mission.nspkg").unwrap();
        assert_eq!(
            c,
            ClassifiedFilename::Asset {
                puid: AssetPuid::new("oil_rig_protection").unwrap(),
                category: AssetCategory::new("mission").unwrap(),
            }
        );
    }

    #[test]
    fn prefab_asset_classifies_with_puid_and_category() {
        let c = ClassifiedFilename::classify("garrisoned_oil_rig.prefab.nspkg").unwrap();
        assert_eq!(
            c,
            ClassifiedFilename::Asset {
                puid: AssetPuid::new("garrisoned_oil_rig").unwrap(),
                category: AssetCategory::new("prefab").unwrap(),
            }
        );
    }

    #[test]
    fn unknown_category_still_classifies_as_an_asset() {
        let c = ClassifiedFilename::classify("thing.some_future_category.nspkg").unwrap();
        assert!(!c.is_package());
        assert_eq!(
            c,
            ClassifiedFilename::Asset {
                puid: AssetPuid::new("thing").unwrap(),
                category: AssetCategory::new("some_future_category").unwrap(),
            }
        );
    }

    #[test]
    fn missing_extension_is_an_explicit_error() {
        assert_eq!(
            ClassifiedFilename::classify("basegame.zip"),
            Err(ClassifyError::MissingExtension)
        );
    }

    #[test]
    fn empty_category_is_an_explicit_error() {
        // "foo..nspkg" -> puid "foo", category ""
        assert!(matches!(
            ClassifiedFilename::classify("foo..nspkg"),
            Err(ClassifyError::InvalidCategory(_))
        ));
    }

    #[test]
    fn empty_asset_puid_is_an_explicit_error() {
        // ".map.nspkg" -> puid "", category "map"
        assert!(matches!(
            ClassifiedFilename::classify(".map.nspkg"),
            Err(ClassifyError::InvalidAssetPuid(_))
        ));
    }

    #[test]
    fn bare_extension_is_an_explicit_error() {
        assert!(matches!(
            ClassifiedFilename::classify(".nspkg"),
            Err(ClassifyError::InvalidPackagePuid(_))
        ));
    }

    #[test]
    fn classify_path_uses_only_the_filename_component() {
        let c = ClassifiedFilename::classify_path(Path::new(
            "/mounted/packages/pebble_sea_islands.map.nspkg",
        ))
        .unwrap();
        assert!(!c.is_package());
    }

    #[test]
    fn classify_path_does_not_require_the_file_to_exist() {
        // No file I/O: an obviously nonexistent path still classifies.
        assert!(
            ClassifiedFilename::classify_path(Path::new("/does/not/exist/anywhere/basegame.nspkg"))
                .is_ok()
        );
    }
}
