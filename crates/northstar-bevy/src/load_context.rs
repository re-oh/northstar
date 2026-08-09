use bevy::asset::{Asset, Handle, LoadContext};
use thiserror::Error;

use northstar_core::{AssetKey, AssetPuid, AssetPuidError, PackageId};

use crate::path::asset_path_string;
use crate::registry::category_for;

/// A Northstar-aware wrapper over Bevy's [`LoadContext`], handed to
/// [`crate::NspkgDecoder::decode`].
///
/// Resolves package-qualified and same-package asset references into typed
/// `Handle<T>` dependencies while preserving Bevy's own dependency tracking
/// (every load still goes through the real `LoadContext`, so
/// `Assets<T>`/asset events/dependency graphs behave exactly as they would
/// for a normal Bevy asset).
pub struct NorthstarLoadContext<'ctx, 'builder> {
    inner: &'builder mut LoadContext<'ctx>,
    owning_package: PackageId,
}

/// Building a `.nspkg` reference inside a decoder failed.
#[derive(Debug, Error)]
pub enum NorthstarReferenceError {
    #[error("invalid asset puid: {0}")]
    InvalidPuid(#[from] AssetPuidError),
    #[error(
        "requested type `{0}` was never registered via register_nspkg_asset; \
         it cannot be referenced from a decoder"
    )]
    UnregisteredType(&'static str),
}

impl<'ctx, 'builder> NorthstarLoadContext<'ctx, 'builder> {
    pub(crate) fn new(inner: &'builder mut LoadContext<'ctx>, owning_package: PackageId) -> Self {
        Self {
            inner,
            owning_package,
        }
    }

    /// The package that owns the asset currently being decoded.
    pub fn owning_package(&self) -> &PackageId {
        &self.owning_package
    }

    /// Load a dependency owned by the *current* package. `load_local` means
    /// the current owning package, not a global search path — a package can
    /// only reference its own assets this way.
    pub fn load_local<A: Asset>(
        &mut self,
        puid: &str,
    ) -> Result<Handle<A>, NorthstarReferenceError> {
        let puid = AssetPuid::new(puid)?;
        let key = AssetKey::new(self.owning_package.clone(), puid);
        self.load(&key)
    }

    /// Load a (possibly cross-package) dependency by its package-qualified
    /// key. Cross-package loading always requires an explicit key —
    /// gameplay and decoder code never construct the underlying
    /// `northstar://` path by hand.
    pub fn load<A: Asset>(&mut self, key: &AssetKey) -> Result<Handle<A>, NorthstarReferenceError> {
        let category = category_for::<A>().ok_or(NorthstarReferenceError::UnregisteredType(
            core::any::type_name::<A>(),
        ))?;
        let filename = format!("{}.{}.nspkg", key.puid(), category);
        let path = asset_path_string(key.package(), &filename);
        Ok(self.inner.load_builder().load::<A>(path))
    }
}
