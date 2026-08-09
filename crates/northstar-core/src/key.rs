use std::fmt;

use crate::package_id::PackageId;
use crate::puid::AssetPuid;

/// The canonical, package-qualified identity of an asset:
/// `(owning_package_identity, asset_puid)`.
///
/// Two `AssetKey`s compare equal only if both the owning package and the
/// PUID match — identical PUIDs owned by different packages are always
/// distinct assets. This is the type gameplay and tooling code should hold
/// onto for asset identity; a bare [`AssetPuid`] is not enough on its own.
///
/// Note this deliberately excludes [`crate::AssetCategory`]: category is
/// classification/type information, not part of an asset's identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetKey {
    package: PackageId,
    puid: AssetPuid,
}

impl AssetKey {
    pub fn new(package: PackageId, puid: AssetPuid) -> Self {
        Self { package, puid }
    }

    pub fn package(&self) -> &PackageId {
        &self.package
    }

    pub fn puid(&self) -> &AssetPuid {
        &self.puid
    }
}

impl fmt::Display for AssetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.package, self.puid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_puid_different_package_are_distinct() {
        let a = AssetKey::new(
            PackageId::new("basegame").unwrap(),
            AssetPuid::new("oil_rig_protection").unwrap(),
        );
        let b = AssetKey::new(
            PackageId::new("some_mod").unwrap(),
            AssetPuid::new("oil_rig_protection").unwrap(),
        );
        assert_ne!(a, b);
    }

    #[test]
    fn same_package_and_puid_are_equal() {
        let a = AssetKey::new(
            PackageId::new("basegame").unwrap(),
            AssetPuid::new("oil_rig_protection").unwrap(),
        );
        let b = AssetKey::new(
            PackageId::new("basegame").unwrap(),
            AssetPuid::new("oil_rig_protection").unwrap(),
        );
        assert_eq!(a, b);
    }
}
