//! A minimal, headless Bevy `App` for testing.
//!
//! Tests for assets, simulation components, and editor data should not
//! require launching the complete game (a window, renderer, audio device,
//! ...). [`NorthstarTestApp`] wires up exactly what [`NorthstarPlugin`]
//! needs — `MinimalPlugins` + `AssetPlugin`, no windowing/rendering
//! plugins — and nothing else, then derefs to [`bevy::app::App`] so it's
//! used exactly like one:
//!
//! ```
//! use northstar_test_app::NorthstarTestApp;
//!
//! let mut app = NorthstarTestApp::new();
//! app.update();
//! ```

use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::app::App;
use bevy::asset::AssetPlugin;

pub use northstar::{NorthstarPhase, NorthstarPlugin};
pub use northstar_bevy::PackageCatalog;

/// A headless [`App`] with [`NorthstarPlugin`] (and therefore the asset,
/// time, and diagnostics foundations) already installed.
///
/// [`NorthstarTestApp::new`] mounts a fresh, empty, per-instance temporary
/// directory as its package catalog — enough for `NorthstarPlugin` to
/// build, without requiring every test that doesn't care about assets to
/// think about one. Tests that *do* care about specific `.nspkg` fixtures
/// should use [`NorthstarTestApp::with_catalog`] pointed at a real fixture
/// directory instead (see `crates/northstar-core/tests/fixtures`).
pub struct NorthstarTestApp {
    app: App,
    // Kept alive only so the temp directory `new()` created is removed on
    // drop; unused when constructed via `with_catalog`.
    _owned_temp_dir: Option<PathBuf>,
}

impl NorthstarTestApp {
    /// A headless app backed by a fresh, empty, throwaway package catalog.
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "northstar-test-app-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create NorthstarTestApp's throwaway catalog directory");

        Self {
            app: Self::bare_app(PackageCatalog::loose_directory(&dir)),
            _owned_temp_dir: Some(dir),
        }
    }

    /// A headless app backed by `catalog` — use this when the test needs
    /// to load real `.nspkg` fixtures.
    ///
    /// Like [`NorthstarTestApp::new`], `Startup` (and therefore
    /// `NorthstarPhase`) has *not* run yet — it runs on the caller's first
    /// `update()`, same as any other freshly constructed `App`. This
    /// matters: a test that needs its own `Startup` systems can still add
    /// them before that first `update()` and have them run in order.
    pub fn with_catalog(catalog: PackageCatalog) -> Self {
        Self {
            app: Self::bare_app(catalog),
            _owned_temp_dir: None,
        }
    }

    fn bare_app(catalog: PackageCatalog) -> App {
        let mut app = App::new();
        app.add_plugins((
            bevy::MinimalPlugins,
            NorthstarPlugin::new(catalog),
            AssetPlugin::default(),
        ));
        app
    }
}

impl Default for NorthstarTestApp {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NorthstarTestApp {
    fn drop(&mut self) {
        if let Some(dir) = &self._owned_temp_dir {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

impl Deref for NorthstarTestApp {
    type Target = App;

    fn deref(&self) -> &App {
        &self.app
    }
}

impl DerefMut for NorthstarTestApp {
    fn deref_mut(&mut self) -> &mut App {
        &mut self.app
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_constructs_and_updates_without_a_window() {
        let mut app = NorthstarTestApp::new();
        app.update();
        app.update();
    }

    #[test]
    fn derefs_to_app_for_normal_bevy_apis() {
        let app = NorthstarTestApp::new();
        // Resource access through Deref, exactly like a plain `App`.
        assert!(app.world().contains_resource::<bevy::asset::AssetServer>());
    }
}
