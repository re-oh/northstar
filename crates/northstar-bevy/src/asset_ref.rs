use std::marker::PhantomData;

use bevy::asset::{Asset, AssetServer, Handle};
use bevy::ecs::system::{Res, SystemParam};

use northstar_core::{AssetPuid, PackageId};

use crate::path::asset_path_string;
use crate::registry::category_for;

/// A strong, package-qualified, typed reference to an asset gameplay code
/// wants to load.
///
/// Building an `AssetRef` never touches the filesystem or the category
/// registry — resolution happens when it is handed to
/// [`NorthstarAssets::load`]. Gameplay code should hold and pass around
/// `AssetRef<T>` rather than a bare `AssetPuid` string or a hand-built
/// `northstar://` path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRef<A: Asset> {
    package: PackageId,
    puid: AssetPuid,
    _asset: PhantomData<fn() -> A>,
}

impl<A: Asset> AssetRef<A> {
    pub fn new(package: PackageId, puid: AssetPuid) -> Self {
        Self {
            package,
            puid,
            _asset: PhantomData,
        }
    }

    pub fn package(&self) -> &PackageId {
        &self.package
    }

    pub fn puid(&self) -> &AssetPuid {
        &self.puid
    }
}

/// The gameplay-facing entry point for loading `.nspkg` assets.
///
/// `NorthstarAssets::load` resolves the owning package and package-local
/// PUID from an [`AssetRef<A>`], looks up the `.nspkg` category `A` was
/// registered under (validating that the requested runtime type has a
/// registered category at all), constructs the internal `northstar://`
/// path, performs a typed Bevy load, and returns an ordinary `Handle<A>`.
/// After that, `A` is consumed exactly like any other Bevy asset — through
/// `Assets<A>` and normal asset events. There is no parallel handle or
/// lifetime system.
#[derive(SystemParam)]
pub struct NorthstarAssets<'w> {
    asset_server: Res<'w, AssetServer>,
}

impl<'w> NorthstarAssets<'w> {
    /// Load `reference`. Panics if `A` was never bound to a category via
    /// `register_nspkg_asset` — that is a static configuration bug, not a
    /// condition callers are expected to recover from at this call site.
    pub fn load<A: Asset>(&self, reference: AssetRef<A>) -> Handle<A> {
        let category = category_for::<A>().unwrap_or_else(|| {
            panic!(
                "AssetRef<{}> requested but that type was never registered via \
                 register_nspkg_asset",
                core::any::type_name::<A>()
            )
        });
        let filename = format!("{}.{}.nspkg", reference.puid, category);
        let path = asset_path_string(&reference.package, &filename);
        self.asset_server.load::<A>(path)
    }
}
