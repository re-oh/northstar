//! Northstar's main entry point.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use bevy::DefaultPlugins;
use bevy::app::{App, PluginGroup};
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{Window, WindowPlugin};

const BACKGROUND: Color = Color::srgb(0.035, 0.04, 0.055);
const SURFACE: Color = Color::srgb(0.075, 0.085, 0.11);
const BORDER: Color = Color::srgb(0.19, 0.21, 0.27);
const TEXT: Color = Color::srgb(0.93, 0.94, 0.98);

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum LaunchTarget {
    Game,
    Editor,
}

impl LaunchTarget {
    fn binary(self) -> &'static str {
        match self {
            Self::Game => "northstar-game",
            Self::Editor => "northstar-editor",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Game => "Northstar",
            Self::Editor => "Northstar Editor",
        }
    }
}

#[derive(Component)]
struct LauncherButton {
    normal: Color,
    hovered: Color,
    pressed: Color,
}

#[derive(Component)]
struct StatusText;

#[derive(Resource, Default)]
struct RunningApplications(HashMap<LaunchTarget, Child>);

type ChangedLaunchButtons = (With<LauncherButton>, Changed<Interaction>);

fn main() {
    northstar_diagnostics::install_panic_hook();
    northstar_diagnostics::init_logging();

    App::new()
        .add_plugins(
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
        )
        .init_resource::<RunningApplications>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (update_button_visuals, launch_selected_target, reap_children),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(BACKGROUND),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: px(420),
                    padding: UiRect::all(px(32)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(14),
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(SURFACE),
                BorderColor::all(BORDER),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("NORTHSTAR"),
                    TextFont::from_font_size(34.0),
                    TextColor(TEXT),
                ));
                card.spawn((
                    Text::new("0.1 dev launcher"),
                    TextFont::from_font_size(14.0),
                    TextColor(TEXT.with_alpha(0.62)),
                    Node {
                        margin: UiRect::bottom(px(16)),
                        ..default()
                    },
                ));
                spawn_launch_button(card, "Launch Northstar", LaunchTarget::Game, true);
                spawn_launch_button(card, "Launch Northstar Editor", LaunchTarget::Editor, false);
                card.spawn((
                    StatusText,
                    Text::new("Ready"),
                    TextFont::from_font_size(13.0),
                    TextColor(TEXT.with_alpha(0.72)),
                    Node {
                        margin: UiRect::top(px(10)),
                        ..default()
                    },
                ));
            });
        });
}

fn spawn_launch_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    target: LaunchTarget,
    primary: bool,
) {
    let normal = if primary {
        Color::srgb(0.18, 0.31, 0.58)
    } else {
        Color::srgb(0.12, 0.14, 0.18)
    };
    let hovered = if primary {
        Color::srgb(0.23, 0.39, 0.70)
    } else {
        Color::srgb(0.18, 0.21, 0.28)
    };
    let pressed = if primary {
        Color::srgb(0.14, 0.25, 0.48)
    } else {
        Color::srgb(0.09, 0.11, 0.15)
    };

    parent
        .spawn((
            Button,
            LauncherButton {
                normal,
                hovered,
                pressed,
            },
            target,
            Node {
                width: percent(100),
                height: px(50),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(normal),
            BorderColor::all(BORDER),
        ))
        .with_child((
            Text::new(label),
            TextFont::from_font_size(17.0),
            TextColor(TEXT),
        ));
}

fn update_button_visuals(
    mut buttons: Query<(&Interaction, &LauncherButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, colors, mut background) in &mut buttons {
        background.0 = match interaction {
            Interaction::None => colors.normal,
            Interaction::Hovered => colors.hovered,
            Interaction::Pressed => colors.pressed,
        };
    }
}

fn launch_selected_target(
    buttons: Query<(&Interaction, &LaunchTarget), ChangedLaunchButtons>,
    mut running: ResMut<RunningApplications>,
    mut status: Query<&mut Text, With<StatusText>>,
) {
    for (interaction, target) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        if running.0.contains_key(target) {
            set_status(
                &mut status,
                format!("{} is already running", target.label()),
            );
            continue;
        }

        match launch(*target) {
            Ok(child) => {
                running.0.insert(*target, child);
                set_status(&mut status, format!("Launched {}", target.label()));
            }
            Err(error) => {
                tracing::error!(%error, target = target.label(), "failed to launch application");
                set_status(
                    &mut status,
                    format!("Could not launch {}: {error}", target.label()),
                );
            }
        }
    }
}

fn reap_children(
    mut running: ResMut<RunningApplications>,
    mut status: Query<&mut Text, With<StatusText>>,
) {
    let mut finished = Vec::new();
    for (target, child) in &mut running.0 {
        match child.try_wait() {
            Ok(Some(exit)) => {
                set_status(
                    &mut status,
                    format!("{} exited with {exit}", target.label()),
                );
                finished.push(*target);
            }
            Ok(None) => {}
            Err(error) => {
                tracing::error!(%error, target = target.label(), "failed to poll application");
                set_status(
                    &mut status,
                    format!("Lost track of {}: {error}", target.label()),
                );
                finished.push(*target);
            }
        }
    }
    for target in finished {
        running.0.remove(&target);
    }
}

fn set_status(status: &mut Query<&mut Text, With<StatusText>>, message: String) {
    if let Ok(mut text) = status.single_mut() {
        **text = message;
    }
}

fn launch(target: LaunchTarget) -> io::Result<Child> {
    let sibling = sibling_binary(target.binary())?;
    if sibling.is_file() {
        return Command::new(sibling).spawn();
    }

    #[cfg(debug_assertions)]
    {
        let root = workspace_root();
        if !root.join("Cargo.toml").is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("development workspace not found at {}", root.display()),
            ));
        }
        Command::new("cargo")
            .args(["run", "-p", target.binary()])
            .current_dir(root)
            .spawn()
    }

    #[cfg(not(debug_assertions))]
    {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("installed {} binary was not found", target.binary()),
        ))
    }
}

fn sibling_binary(binary: &str) -> io::Result<PathBuf> {
    let current = std::env::current_exe()?;
    let directory = current.parent().unwrap_or_else(|| Path::new("."));
    Ok(directory.join(format!("{binary}{}", std::env::consts::EXE_SUFFIX)))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
