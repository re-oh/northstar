# Assets: a practical guide

`docs/architecture.md` covers *why* the `.nspkg`/package/asset system is
shaped the way it is. This document is the shorter, practical companion:
how to actually use it from each crate. Read `architecture.md` first if
something here doesn't make sense on its own.

## Mounting a package catalog

```rust,ignore
use northstar_bevy::{NorthstarAssetPlugin, PackageCatalog};

app.add_plugins(
    NorthstarAssetPlugin::new(PackageCatalog::loose_directory("assets/packages")),
);
```

Must be added **before** `AssetPlugin` (`DefaultPlugins`/`MinimalPlugins` +
`AssetPlugin`, in that order) — see `NorthstarAssetPlugin`'s own doc
comment for why. If you're using [`NorthstarPlugin`] (which most code
should be) or [`NorthstarTestApp`], this is already handled.

Only the loose-development-directory backend exists today: one
subdirectory per mounted `PackageId` under the catalog root. `assets/packages/basegame/`
holds `basegame`'s files; a path like `northstar://basegame/pebble_sea_islands.map.nspkg`
resolves there.

## Registering a category

Every `.nspkg` category needs an engine-owned type + decoder before
anything with that category can load:

```rust,ignore
app.register_nspkg_asset::<MapAsset, MapDecoder>("map", MapDecoder);
```

Called **after** `AssetPlugin` has built (it calls `init_asset::<MapAsset>()`
internally). Panics if `"map"` or `MapAsset` was already registered to
something else — see `docs/errors.md` on when panicking is the right call.

## Loading an asset

Gameplay/editor code:

```rust,ignore
fn spawn_map(assets: NorthstarAssets) {
    let reference = AssetRef::<MapAsset>::new(
        PackageId::new("basegame").unwrap(),
        AssetPuid::new("pebble_sea_islands").unwrap(),
    );
    let handle: Handle<MapAsset> = assets.load(reference);
}
```

Inside a decoder, resolving a dependency:

```rust,ignore
impl NspkgDecoder for MissionDecoder {
    fn decode(&self, container: &ContainerReader, puid: &AssetPuid, ctx: &mut NorthstarLoadContext) -> Result<MissionAsset, Self::Error> {
        let map = ctx.load_local::<MapAsset>("pebble_sea_islands")?; // same package
        // or, cross-package: ctx.load::<MapAsset>(&some_asset_key)?;
        ...
    }
}
```

Never build a `northstar://` path string by hand in either case — that's
exactly what `AssetRef`/`NorthstarAssets`/`NorthstarLoadContext` exist to
prevent (see `architecture.md`'s Bevy-integration section for why untyped
loading isn't part of the supported API).

## Inspecting `.nspkg` files without Bevy

```sh
northstar-dev assets classify some_package.map.nspkg   # filename only, no I/O
northstar-dev assets inspect  some_package.map.nspkg   # reads + parses the container
northstar-dev assets pack-test /tmp/example.map.nspkg  # writes a deterministic test fixture
northstar-dev assets unpack-test /tmp/example.map.nspkg /tmp/out/
```

`northstar-dev packages` (catalog-level inspection: listing mounted
packages, showing resolved backing storage) and `northstar-dev validate`
(recursive `.nspkg` validation across a directory) are not implemented yet
— see their own `--help` output for current scope.

## Fixtures and testing

`crates/northstar-core/tests/fixtures/{valid,corrupted}/` has small,
deterministic, checked-in `.nspkg` files — regenerate with
`cargo run --example generate_fixtures -p northstar-core` after a format
change. `crates/northstar-bevy/src/testing.rs` has two trivial registered
asset types (`TestLeafAsset`, `TestParentAsset`, with one dependency
between them) proving the Bevy dispatch path end-to-end — see
`crates/northstar-bevy/tests/nspkg_loading.rs` for how they're used.
`crates/northstar-core/fuzz/` has fuzz targets for the filename classifier
and container parser (see its `README.md`).

[`NorthstarPlugin`]: ../crates/northstar/src/plugin.rs
[`NorthstarTestApp`]: ../crates/northstar-test-app/src/lib.rs
