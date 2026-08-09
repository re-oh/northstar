use std::path::Path;

use bevy::asset::LoadContext;

use northstar_core::PackageId;

use crate::plugin::SOURCE_NAME;

/// Build the logical `northstar://<package>/<filename>` path for one asset.
///
/// This is the only place that constructs Northstar's internal path shape —
/// gameplay and decoder code go through [`crate::AssetRef`] and
/// [`crate::NorthstarLoadContext`] instead of building strings by hand.
pub(crate) fn asset_path_string(package: &PackageId, filename: &str) -> String {
    format!("{SOURCE_NAME}://{package}/{filename}")
}

/// Recover the owning [`PackageId`] from a loader's [`LoadContext`].
///
/// Mounted packages are one subdirectory per [`PackageId`] under the
/// catalog's loose development directory (see
/// [`crate::catalog::PackageCatalog`]), so the owning package is always the
/// first path component of the asset's path within the `northstar` source.
///
/// # Panics
///
/// Panics if the path has no first component. This can only happen for an
/// asset loaded directly at the mount root, which
/// [`crate::plugin::NorthstarAssetPlugin`]'s source layout never produces.
pub(crate) fn package_id_from_load_context(load_context: &LoadContext<'_>) -> PackageId {
    package_id_from_path(load_context.path().path())
}

pub(crate) fn package_id_from_path(path: &Path) -> PackageId {
    let first = path
        .components()
        .next()
        .unwrap_or_else(|| {
            panic!(
                "asset path \"{}\" has no package directory component",
                path.display()
            )
        })
        .as_os_str()
        .to_string_lossy();
    PackageId::new(first.into_owned()).unwrap_or_else(|e| {
        panic!(
            "asset path \"{}\" has an invalid package directory component: {e}",
            path.display()
        )
    })
}
