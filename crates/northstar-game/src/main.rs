//! The minimal Northstar executable: it opens a window, installs
//! [`NorthstarPlugin`] plus Bevy's `DefaultPlugins`, and exits cleanly when
//! the window is closed. Nothing else — see backlog item 1 in
//! `northstar-asset-foundation-agent-brief.md`'s follow-up plan. This is
//! the thing to `cargo run -p northstar-game` to sanity-check the
//! foundation actually boots.

use bevy::DefaultPlugins;
use bevy::app::App;
use bevy::app::PluginGroup;
use bevy::log::LogPlugin;

use northstar::NorthstarPlugin;
use northstar_bevy::PackageCatalog;

fn main() {
    // Install these before any `App` exists so they cover bootstrap-time
    // panics and the earliest log lines too — see
    // `northstar_diagnostics`'s crate docs.
    northstar_diagnostics::install_panic_hook();
    northstar_diagnostics::init_logging();

    App::new()
        .add_plugins((
            // Must precede `DefaultPlugins`: installs the `northstar://`
            // asset source, which Bevy's `AssetPlugin` (part of
            // `DefaultPlugins`) needs to already know about when it builds.
            NorthstarPlugin::new(PackageCatalog::loose_directory("assets/packages")),
            // Bevy's own `LogPlugin` also tries to install a global
            // `tracing` subscriber, which conflicts with (and loses to,
            // noisily) the one `northstar_diagnostics::init_logging` just
            // installed above. Disable it so northstar-diagnostics stays
            // the single source of truth for logging setup everywhere —
            // `northstar-game`, `northstar-dev`, and `NorthstarTestApp`
            // alike.
            DefaultPlugins.build().disable::<LogPlugin>(),
        ))
        .run();
}
