use bevy::app::{App, Plugin};
use bevy::asset::AssetApp;
use bevy::asset::io::AssetSourceBuilder;

use crate::catalog::PackageCatalog;

/// The named Bevy asset source Northstar mounts its package catalog under.
/// Asset paths look like `northstar://<package-identity>/<file>`.
pub const SOURCE_NAME: &str = "northstar";

/// Northstar's Bevy integration entry point.
///
/// Registers the `northstar://` asset source backed by `catalog`. **Must be
/// added before `AssetPlugin`** (typically part of
/// [`bevy::prelude::DefaultPlugins`]) — Bevy builds registered sources when
/// `AssetPlugin` itself builds, so a source registered afterwards is
/// invisible to the running `AssetServer`:
///
/// ```rust,ignore
/// App::new()
///     .add_plugins((
///         NorthstarAssetPlugin::new(PackageCatalog::loose_directory("assets/packages")),
///         DefaultPlugins,
///     ))
///     .register_nspkg_asset::<TestMapAsset, TestMapDecoder>("test_map", TestMapDecoder)
///     .run();
/// ```
///
/// This first slice only implements the loose-development-directory catalog
/// backend (plain files under `AssetSourceBuilder::platform_default`) — see
/// `docs/architecture.md` for the deferred Steam/archive backends.
pub struct NorthstarAssetPlugin {
    catalog: PackageCatalog,
}

impl NorthstarAssetPlugin {
    pub fn new(catalog: PackageCatalog) -> Self {
        Self { catalog }
    }
}

impl Plugin for NorthstarAssetPlugin {
    fn build(&self, app: &mut App) {
        let root = self.catalog.root().to_string_lossy().into_owned();
        app.register_asset_source(
            SOURCE_NAME,
            AssetSourceBuilder::platform_default(&root, None),
        );
    }
}
