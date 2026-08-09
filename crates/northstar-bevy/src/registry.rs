//! The category → asset-type registry.
//!
//! Bevy's [`LoadContext`](bevy::asset::LoadContext) does not expose the
//! World or its resources to an in-flight [`AssetLoader`](bevy::asset::AssetLoader)
//! (see `docs/architecture.md`'s notes on this crate's deferred decisions),
//! so there is no ECS-resource path from [`crate::NorthstarLoadContext`]
//! (running inside a decoder) back to "which category string did the
//! `MapAsset` type get registered under". This registry is instead a small
//! process-global table, written once per type at
//! [`NspkgAssetApp::register_nspkg_asset`] time and read by both
//! [`crate::NorthstarLoadContext`] and [`crate::NorthstarAssets`] to build
//! the `.nspkg` filename for a requested asset type. It is a deliberately
//! provisional choice, isolated behind this module so it can be replaced
//! (e.g. if a future Bevy version threads resources through `LoadContext`)
//! without touching call sites.

use std::any::TypeId;
use std::sync::{OnceLock, RwLock};

use bevy::app::App;
use bevy::asset::{Asset, AssetApp};
use bevy::platform::collections::HashMap;

use northstar_core::AssetCategory;

use crate::loader::{NspkgDecoder, NspkgLoader};

#[derive(Debug, Clone)]
struct RegisteredCategory {
    category: AssetCategory,
    asset_type_name: &'static str,
}

#[derive(Default)]
struct Registry {
    by_type: HashMap<TypeId, RegisteredCategory>,
    type_by_category: HashMap<AssetCategory, TypeId>,
}

fn registry() -> &'static RwLock<Registry> {
    static REGISTRY: OnceLock<RwLock<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(Registry::default()))
}

/// Look up the `.nspkg` category `A` was registered under, if any.
pub(crate) fn category_for<A: Asset>() -> Option<AssetCategory> {
    registry()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .by_type
        .get(&TypeId::of::<A>())
        .map(|r| r.category.clone())
}

/// Register `A` under `category`. Idempotent when called again with the
/// exact same `(A, category)` pair (this can legitimately happen across
/// multiple `App`s in one process, e.g. in tests); panics on a genuine
/// conflict — either type already bound to a different category, or the
/// category already claimed by a different type.
fn register<A: Asset>(category: AssetCategory) {
    let type_id = TypeId::of::<A>();
    let asset_type_name = core::any::type_name::<A>();
    let mut reg = registry().write().unwrap_or_else(|e| e.into_inner());

    if let Some(existing) = reg.by_type.get(&type_id) {
        if existing.category == category {
            return; // idempotent re-registration
        }
        panic!(
            "asset type `{asset_type_name}` is already registered under nspkg category \
             `{}`, cannot also register it under `{category}`",
            existing.category
        );
    }
    if let Some(existing_type) = reg.type_by_category.get(&category)
        && *existing_type != type_id
    {
        let existing_name = reg
            .by_type
            .get(existing_type)
            .map(|r| r.asset_type_name)
            .unwrap_or("<unknown>");
        panic!(
            "nspkg category `{category}` is already registered to asset type \
             `{existing_name}`, cannot also register it to `{asset_type_name}` \
             — a category maps to exactly one runtime asset type"
        );
    }

    reg.type_by_category.insert(category.clone(), type_id);
    reg.by_type.insert(
        type_id,
        RegisteredCategory {
            category,
            asset_type_name,
        },
    );
}

/// Extension trait for registering an `.nspkg` asset category with the app.
///
/// Registration is engine code only: mod content configures data understood
/// by an already-registered category, but can never register a new one — see
/// `docs/architecture.md`'s data-only mod boundary.
pub trait NspkgAssetApp {
    /// Bind the `.nspkg` category named `category` to runtime asset type `A`,
    /// decoded by `decoder`. Panics if `A` or `category` was already bound
    /// to something else — that is a static configuration bug, not a
    /// runtime condition callers are expected to recover from.
    ///
    /// Must be called after `AssetPlugin` (typically part of
    /// [`bevy::prelude::DefaultPlugins`]) has been added, since it
    /// initializes `Assets<A>`.
    fn register_nspkg_asset<A, D>(&mut self, category: &str, decoder: D) -> &mut Self
    where
        A: Asset,
        D: NspkgDecoder<Asset = A>;
}

impl NspkgAssetApp for App {
    fn register_nspkg_asset<A, D>(&mut self, category: &str, decoder: D) -> &mut Self
    where
        A: Asset,
        D: NspkgDecoder<Asset = A>,
    {
        let category = AssetCategory::new(category)
            .unwrap_or_else(|e| panic!("register_nspkg_asset: invalid category {category:?}: {e}"));

        register::<A>(category.clone());
        self.init_asset::<A>();
        self.register_asset_loader(NspkgLoader::new(category, decoder));

        self
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::Asset;
    use bevy::reflect::TypePath;

    use super::*;

    #[derive(Asset, TypePath)]
    struct NeverRegisteredAsset;

    #[test]
    fn unregistered_type_has_no_category() {
        assert!(category_for::<NeverRegisteredAsset>().is_none());
    }

    #[derive(Asset, TypePath)]
    struct RepeatRegistrationAsset;

    #[test]
    fn re_registering_the_same_type_and_category_is_idempotent() {
        register::<RepeatRegistrationAsset>(AssetCategory::new("repeatable").unwrap());
        // Simulates a second `App` in the same test process registering the
        // same type under the same category — must not panic.
        register::<RepeatRegistrationAsset>(AssetCategory::new("repeatable").unwrap());
        assert_eq!(
            category_for::<RepeatRegistrationAsset>().unwrap(),
            AssetCategory::new("repeatable").unwrap()
        );
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn conflicting_category_for_the_same_type_panics() {
        #[derive(Asset, TypePath)]
        struct ConflictAsset;

        register::<ConflictAsset>(AssetCategory::new("first").unwrap());
        register::<ConflictAsset>(AssetCategory::new("second").unwrap());
    }
}
