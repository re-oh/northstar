use std::path::{Path, PathBuf};

/// Where Northstar resolves `northstar://<package-identity>/...` paths to on
/// disk.
///
/// This first slice implements exactly one backend: a loose development
/// directory containing one subdirectory per mounted [`PackageId`]. Steam
/// installations, a fallback installation location, and archive members are
/// deliberately out of scope here — see `docs/architecture.md` — but nothing
/// in [`crate::plugin::NorthstarAssetPlugin`] assumes this is the only
/// backend that will ever exist; gameplay code never sees this path at all,
/// only the logical `northstar://` locators built by [`crate::AssetRef`].
///
/// [`PackageId`]: northstar_core::PackageId
#[derive(Debug, Clone)]
pub struct PackageCatalog {
    root: PathBuf,
}

impl PackageCatalog {
    /// A catalog backed by a loose directory of mounted packages, e.g.
    /// `root/basegame/pebble_sea_islands.map.nspkg`.
    pub fn loose_directory(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
