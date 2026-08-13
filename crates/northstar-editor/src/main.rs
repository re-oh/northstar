//! Minimal Northstar editor application shell.

use bevy::DefaultPlugins;
use bevy::app::{App, PluginGroup};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin};
use northstar::NorthstarPlugin;
use northstar_bevy::PackageCatalog;
use northstar_ui::NorthstarUiPlugin;

fn main() {
    northstar_diagnostics::install_panic_hook();
    northstar_diagnostics::init_logging();

    App::new()
        .add_plugins((
            NorthstarPlugin::new(PackageCatalog::loose_directory("assets/packages")),
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Northstar Editor".into(),
                        ..default()
                    }),
                    ..default()
                })
                .disable::<LogPlugin>(),
            NorthstarUiPlugin::default(),
        ))
        .add_systems(Startup, setup_camera)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
