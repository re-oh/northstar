use bevy::app::{App, Plugin, Startup};
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;

use crate::build_info::BuildInfo;
use crate::targets;

/// Northstar's startup diagnostics: logs one build-info banner, and
/// optionally wires up Bevy's own frame-time diagnostics.
///
/// This does **not** call [`crate::init_logging`] or
/// [`crate::install_panic_hook`] itself — those need to run before any
/// `App` exists (to catch bootstrap-time panics and early log lines), so
/// callers install them first and only then add this plugin. See
/// `crates/northstar-game/src/main.rs` for the intended order.
#[derive(Debug, Clone, Copy)]
pub struct NorthstarDiagnosticsPlugin {
    /// Add Bevy's [`FrameTimeDiagnosticsPlugin`] (frame time / FPS).
    pub frame_time: bool,
}

impl Default for NorthstarDiagnosticsPlugin {
    fn default() -> Self {
        Self { frame_time: true }
    }
}

impl Plugin for NorthstarDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        if self.frame_time && !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.add_systems(Startup, log_startup_banner);
    }
}

fn log_startup_banner() {
    let build_info = BuildInfo::current();
    tracing::info!(target: targets::BOOTSTRAP, %build_info, "starting");
}
