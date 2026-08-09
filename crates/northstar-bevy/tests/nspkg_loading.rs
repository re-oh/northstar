//! End-to-end proof that the Bevy integration actually dispatches: builds
//! real `.nspkg` files on disk, mounts them through `NorthstarAssetPlugin`,
//! and drives a real `App` until both the trivial test category and its
//! one cross-asset dependency finish loading.
//!
//! Covers required tests 11 and 12 from the agent brief.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::app::{App, Startup};
use bevy::asset::{AssetApp, AssetPlugin, AssetServer, Assets, Handle, LoadState};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;

use northstar_bevy::testing::{TestLeafAsset, TestLeafDecoder, TestParentAsset, TestParentDecoder};
use northstar_bevy::{NorthstarAssetPlugin, NorthstarAssets, NspkgAssetApp, PackageCatalog};
use northstar_core::{AssetCategory, AssetPuid, ClassifiedFilename, ContainerWriter, PackageId};

/// A directory under the OS temp dir, unique per test process, cleaned up
/// on drop. Deliberately dependency-free (no `tempfile` crate) — this is
/// test-only plumbing, not part of the crate's design surface.
struct TempMountRoot(PathBuf);

impl TempMountRoot {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("northstar-bevy-test-{label}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempMountRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_asset(root: &Path, package: &str, puid: &str, category: &str, metadata: &[u8]) {
    let identity = ClassifiedFilename::Asset {
        puid: AssetPuid::new(puid).unwrap(),
        category: AssetCategory::new(category).unwrap(),
    };
    let bytes = ContainerWriter::new(identity)
        .with_metadata(metadata.to_vec())
        .encode();
    let dir = root.join(package);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{puid}.{category}.nspkg")), bytes).unwrap();
}

#[derive(Resource, Default)]
struct LoadedHandle(Option<Handle<TestParentAsset>>);

fn kick_off_load(mut loaded: ResMut<LoadedHandle>, assets: NorthstarAssets) {
    let reference = northstar_bevy::AssetRef::<TestParentAsset>::new(
        PackageId::new("basegame").unwrap(),
        AssetPuid::new("parent_one").unwrap(),
    );
    loaded.0 = Some(assets.load(reference));
}

/// Runs `app.update()` until `handle`'s [`LoadState`] is no longer
/// [`LoadState::Loading`]/[`LoadState::NotLoaded`], or `max_updates` is hit.
fn drain_until_settled(
    app: &mut App,
    handle: &Handle<impl bevy::asset::Asset>,
    max_updates: usize,
) -> LoadState {
    for _ in 0..max_updates {
        app.update();
        let state = app.world().resource::<AssetServer>().load_state(handle);
        if !matches!(state, LoadState::Loading | LoadState::NotLoaded) {
            return state;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app.world().resource::<AssetServer>().load_state(handle)
}

fn new_test_app(root: &Path) -> App {
    let mut app = App::new();
    app.add_plugins((
        bevy::MinimalPlugins,
        NorthstarAssetPlugin::new(PackageCatalog::loose_directory(root)),
        AssetPlugin::default(),
    ));
    app.register_nspkg_asset::<TestLeafAsset, TestLeafDecoder>("test_leaf", TestLeafDecoder);
    app.register_nspkg_asset::<TestParentAsset, TestParentDecoder>(
        "test_parent",
        TestParentDecoder,
    );
    app
}

#[test]
fn trivial_category_and_its_dependency_load_through_the_registered_handlers() {
    let root = TempMountRoot::new("dispatch");
    write_asset(
        &root.0,
        "basegame",
        "leaf_one",
        "test_leaf",
        b"hello from leaf",
    );
    write_asset(
        &root.0,
        "basegame",
        "parent_one",
        "test_parent",
        b"a label\nleaf_one",
    );

    let mut app = new_test_app(&root.0);
    app.init_resource::<LoadedHandle>();
    app.add_systems(Startup, kick_off_load);

    let parent_handle = {
        // Run once so Startup executes and the handle resource is populated.
        app.update();
        app.world()
            .resource::<LoadedHandle>()
            .0
            .clone()
            .expect("kick_off_load must populate LoadedHandle")
    };

    let state = drain_until_settled(&mut app, &parent_handle, 400);
    assert!(
        matches!(state, LoadState::Loaded),
        "expected TestParentAsset to load, got {state:?}"
    );

    let (label, leaf_handle) = {
        let parents = app.world().resource::<Assets<TestParentAsset>>();
        let parent = parents
            .get(&parent_handle)
            .expect("loaded asset must be present");
        (parent.label.clone(), parent.leaf.clone())
    };
    assert_eq!(label, "a label");

    // The dependency: the parent's leaf handle must itself resolve to the
    // expected typed asset, proving `load_local` produced a real, working
    // Bevy dependency (not just an opaque puid string).
    let leaf_state = drain_until_settled(&mut app, &leaf_handle, 400);
    assert!(
        matches!(leaf_state, LoadState::Loaded),
        "expected the parent's leaf dependency to load, got {leaf_state:?}"
    );
    let leaves = app.world().resource::<Assets<TestLeafAsset>>();
    let leaf = leaves
        .get(&leaf_handle)
        .expect("loaded leaf must be present");
    assert_eq!(leaf.value, "hello from leaf");
}

#[test]
fn unregistered_category_fails_with_an_actionable_error_instead_of_silent_misclassification() {
    let root = TempMountRoot::new("unregistered");
    // A perfectly well-formed test_leaf asset — the file is fine. The point
    // is that *this app* below never calls `register_nspkg_asset` for it.
    write_asset(
        &root.0,
        "basegame",
        "leaf_one",
        "test_leaf",
        b"hello from leaf",
    );

    // Classification itself must still succeed on the filename alone: an
    // unregistered category is not misclassified as a complete package.
    let classified = ClassifiedFilename::classify("leaf_one.test_leaf.nspkg")
        .expect("an unrecognized-but-well-formed category must still classify");
    assert!(!classified.is_package());

    // But *loading* it must fail cleanly. Deliberately do NOT call
    // `register_nspkg_asset::<TestLeafAsset, _>` on this app — that is
    // exactly the "category classifies fine, but nothing engine-owned
    // claims it yet" situation the brief requires an actionable error for.
    let mut app = App::new();
    app.add_plugins((
        bevy::MinimalPlugins,
        NorthstarAssetPlugin::new(PackageCatalog::loose_directory(&root.0)),
        AssetPlugin::default(),
    ));
    // Asset *types* must still be initialized to allocate a `Handle<T>` at
    // all — that's independent of whether a *loader* is registered for
    // them, which is the thing this test is actually about.
    app.init_asset::<TestLeafAsset>();

    let handle: Handle<TestLeafAsset> = app
        .world()
        .resource::<AssetServer>()
        .load("northstar://basegame/leaf_one.test_leaf.nspkg");

    let state = drain_until_settled(&mut app, &handle, 200);
    assert!(
        matches!(state, LoadState::Failed(_)),
        "loading an asset type with no registered nspkg loader must fail explicitly, got {state:?}"
    );
}
