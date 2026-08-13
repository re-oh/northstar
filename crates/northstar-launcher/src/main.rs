//! Northstar's main entry point.

use std::path::{Path, PathBuf};
use std::process::Command;

use bevy::DefaultPlugins;
use bevy::app::{App, PluginGroup};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin};
use northstar_ui::prelude::*;

#[derive(Component, Clone, Copy)]
enum LaunchTarget {
    Game,
    Editor,
}

type ChangedLaunchButtons = (With<UiButton>, Changed<Interaction>);

fn main() {
    northstar_diagnostics::install_panic_hook();
    northstar_diagnostics::init_logging();

    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Northstar Launcher".into(),
                        resolution: (760, 480).into(),
                        ..default()
                    }),
                    ..default()
                })
                .disable::<LogPlugin>(),
            NorthstarUiPrimitivesPlugin,
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, launch_selected_target)
        .run();
}

fn setup(mut commands: Commands, theme: Res<UiTheme>) {
    commands.spawn(Camera2d);
    commands
        .spawn(UiSurfaceBundle::full_screen(&theme))
        .with_children(|root| {
            let mut card_stack = UiStackBundle::column(14.0);
            card_stack.node.width = px(420);
            card_stack.node.padding = UiRect::all(px(32));
            card_stack.node.align_items = AlignItems::Center;
            card_stack.node.border = UiRect::all(px(1));
            root.spawn((
                card_stack,
                BackgroundColor(theme.surface),
                BorderColor::all(theme.border),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("NORTHSTAR"),
                    TextFont::from_font_size(34.0),
                    TextColor(theme.text),
                ));
                card.spawn((
                    Text::new("0.1 dev launcher"),
                    TextFont::from_font_size(14.0),
                    TextColor(theme.text.with_alpha(0.62)),
                    Node {
                        margin: UiRect::bottom(px(16)),
                        ..default()
                    },
                ));
                spawn_launch_button(
                    card,
                    &theme,
                    "Launch Northstar",
                    LaunchTarget::Game,
                    UiButtonTone::Primary,
                );
                spawn_launch_button(
                    card,
                    &theme,
                    "Launch Northstar Editor",
                    LaunchTarget::Editor,
                    UiButtonTone::Secondary,
                );
            });
        });
}

fn spawn_launch_button(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    label: &str,
    target: LaunchTarget,
    tone: UiButtonTone,
) {
    parent
        .spawn((UiButtonBundle::new(tone, theme), target))
        .with_child((
            Text::new(label),
            TextFont::from_font_size(17.0),
            TextColor(theme.text),
        ));
}

fn launch_selected_target(buttons: Query<(&Interaction, &LaunchTarget), ChangedLaunchButtons>) {
    for (interaction, target) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (binary, package) = match target {
            LaunchTarget::Game => ("northstar-game", "northstar-game"),
            LaunchTarget::Editor => ("northstar-editor", "northstar-editor"),
        };
        if let Err(error) = launch(binary, package) {
            tracing::error!(%error, %package, "failed to launch Northstar application");
        }
    }
}

fn launch(binary: &str, package: &str) -> std::io::Result<()> {
    let sibling = sibling_binary(binary)?;
    if sibling.is_file() {
        Command::new(sibling).spawn()?;
    } else {
        Command::new("cargo")
            .args(["run", "-p", package])
            .current_dir(workspace_root())
            .spawn()?;
    }
    Ok(())
}

fn sibling_binary(binary: &str) -> std::io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    let directory = current.parent().unwrap_or_else(|| Path::new("."));
    Ok(directory.join(format!("{binary}{}", std::env::consts::EXE_SUFFIX)))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
