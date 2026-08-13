//! Fast-moving UI experiments for Northstar.
//!
//! Hover a panel and press `Q` to split it, or `E` to remove it. This is
//! intentionally a small foundation, not a finished window manager.

use std::collections::HashMap;

use bevy::prelude::*;

const TITLE_BAR_HEIGHT: f32 = 30.0;
const BASE_SATURATION: f32 = 0.48;
const BASE_VALUE: f32 = 0.62;

/// Installs the current tiled-panel prototype.
pub struct NorthstarUiPlugin;

impl Plugin for NorthstarUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PanelLayout>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (spawn_panel_on_q, remove_panel_on_e, update_panel_hover),
            );
    }
}

/// Stable identity for a panel in the current process.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PanelId(pub u64);

/// A panel's normalized bounds, measured from the top-left of the window.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A deduplicated point in the panel-edge graph.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelGeometryPoint {
    pub position: Vec2,
    pub is_intersection: bool,
    pub is_edge_midpoint: bool,
}

#[derive(Component)]
struct PanelColor {
    hue: f32,
}

#[derive(Component)]
struct UiRoot;

#[derive(Resource, Default)]
struct PanelLayout {
    next_id: u64,
}

fn setup(mut commands: Commands, mut layout: ResMut<PanelLayout>) {
    commands.spawn(Camera2d);
    let root = commands
        .spawn((
            UiRoot,
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::BLACK),
        ))
        .id();

    spawn_panel(
        &mut commands,
        root,
        &mut layout,
        PanelBounds {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
    );
}

fn spawn_panel_on_q(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    root: Single<Entity, With<UiRoot>>,
    mut layout: ResMut<PanelLayout>,
    mut panels: Query<(&Interaction, &mut PanelBounds, &mut Node), With<PanelId>>,
) {
    if !keys.just_pressed(KeyCode::KeyQ) {
        return;
    }

    for (interaction, mut bounds, mut node) in &mut panels {
        if !matches!(*interaction, Interaction::Hovered | Interaction::Pressed) {
            continue;
        }

        let (existing_bounds, new_bounds) = split_bounds(*bounds);
        *bounds = existing_bounds;
        apply_bounds(&mut node, existing_bounds);
        spawn_panel(&mut commands, *root, &mut layout, new_bounds);
        break;
    }
}

fn remove_panel_on_e(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    panels: Query<(Entity, &Interaction), With<PanelId>>,
) {
    if !keys.just_pressed(KeyCode::KeyE) || panels.iter().count() <= 1 {
        return;
    }

    if let Some((entity, _)) = panels.iter().find(|(_, interaction)| {
        matches!(**interaction, Interaction::Hovered | Interaction::Pressed)
    }) {
        commands.entity(entity).despawn();
        commands.queue(|world: &mut World| rebuild_layout(world));
    }
}

fn spawn_panel(
    commands: &mut Commands,
    root: Entity,
    layout: &mut PanelLayout,
    bounds: PanelBounds,
) {
    let id = PanelId(layout.next_id);
    layout.next_id += 1;
    let hue = panel_hue(id.0);

    let panel = commands
        .spawn((
            id,
            bounds,
            PanelColor { hue },
            Interaction::None,
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                left: percent(bounds.x * 100.0),
                top: percent(bounds.y * 100.0),
                width: percent(bounds.width * 100.0),
                height: percent(bounds.height * 100.0),
                ..default()
            },
            BorderColor::all(Color::BLACK.with_alpha(0.65)),
            BackgroundColor(panel_color(hue, false)),
        ))
        .with_children(|panel| {
            panel
                .spawn((
                    Node {
                        width: percent(100),
                        height: px(TITLE_BAR_HEIGHT),
                        min_height: px(TITLE_BAR_HEIGHT),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(px(10)),
                        ..default()
                    },
                    BackgroundColor(Color::BLACK.with_alpha(0.24)),
                ))
                .with_child((
                    Text::new(format!("Panel {}", id.0)),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::WHITE),
                ));
        })
        .id();
    commands.entity(root).add_child(panel);

    commands.queue(|world: &mut World| rebuild_geometry(world));
}

fn rebuild_layout(world: &mut World) {
    let mut panels: Vec<(Entity, PanelId)> = world
        .query::<(Entity, &PanelId)>()
        .iter(world)
        .map(|(entity, id)| (entity, *id))
        .collect();
    panels.sort_by_key(|(_, id)| id.0);

    let mut bounds = vec![PanelBounds {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    }];
    while bounds.len() < panels.len() {
        let index = bounds
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| (a.width * a.height).total_cmp(&(b.width * b.height)))
            .map(|(index, _)| index)
            .unwrap_or(0);
        let original = bounds[index];
        let (first, second) = split_bounds(original);
        bounds[index] = first;
        bounds.push(second);
    }

    for ((entity, _), bounds) in panels.into_iter().zip(bounds.iter().copied()) {
        let mut panel = world.entity_mut(entity);
        panel.insert(bounds);
        let mut node = panel.get_mut::<Node>().expect("panels always have a Node");
        apply_bounds(&mut node, bounds);
    }

    rebuild_geometry_points(world, &bounds);
}

fn apply_bounds(node: &mut Node, bounds: PanelBounds) {
    node.left = percent(bounds.x * 100.0);
    node.top = percent(bounds.y * 100.0);
    node.width = percent(bounds.width * 100.0);
    node.height = percent(bounds.height * 100.0);
}

fn rebuild_geometry(world: &mut World) {
    let bounds: Vec<PanelBounds> = world.query::<&PanelBounds>().iter(world).copied().collect();
    rebuild_geometry_points(world, &bounds);
}

fn split_bounds(bounds: PanelBounds) -> (PanelBounds, PanelBounds) {
    if bounds.width >= bounds.height {
        let half = bounds.width * 0.5;
        (
            PanelBounds {
                width: half,
                ..bounds
            },
            PanelBounds {
                x: bounds.x + half,
                width: half,
                ..bounds
            },
        )
    } else {
        let half = bounds.height * 0.5;
        (
            PanelBounds {
                height: half,
                ..bounds
            },
            PanelBounds {
                y: bounds.y + half,
                height: half,
                ..bounds
            },
        )
    }
}

fn rebuild_geometry_points(world: &mut World, panels: &[PanelBounds]) {
    let old_points: Vec<Entity> = world
        .query_filtered::<Entity, With<PanelGeometryPoint>>()
        .iter(world)
        .collect();
    for entity in old_points {
        world.despawn(entity);
    }

    let mut points: HashMap<(i32, i32), PanelGeometryPoint> = HashMap::new();
    for bounds in panels {
        let left = bounds.x;
        let right = bounds.x + bounds.width;
        let top = bounds.y;
        let bottom = bounds.y + bounds.height;
        let middle_x = (left + right) * 0.5;
        let middle_y = (top + bottom) * 0.5;

        for position in [
            Vec2::new(left, top),
            Vec2::new(right, top),
            Vec2::new(left, bottom),
            Vec2::new(right, bottom),
        ] {
            merge_geometry_point(&mut points, position, true, false);
        }
        for position in [
            Vec2::new(middle_x, top),
            Vec2::new(middle_x, bottom),
            Vec2::new(left, middle_y),
            Vec2::new(right, middle_y),
        ] {
            merge_geometry_point(&mut points, position, false, true);
        }
    }

    world.spawn_batch(points.into_values());
}

fn merge_geometry_point(
    points: &mut HashMap<(i32, i32), PanelGeometryPoint>,
    position: Vec2,
    is_intersection: bool,
    is_edge_midpoint: bool,
) {
    // Quantization makes shared points reliably identical even if their
    // coordinates arrived through slightly different floating-point math.
    let key = (
        (position.x * 1_000_000.0).round() as i32,
        (position.y * 1_000_000.0).round() as i32,
    );
    let point = points.entry(key).or_insert(PanelGeometryPoint {
        position,
        is_intersection: false,
        is_edge_midpoint: false,
    });
    point.is_intersection |= is_intersection;
    point.is_edge_midpoint |= is_edge_midpoint;
}

fn update_panel_hover(
    mut panels: Query<(&Interaction, &PanelColor, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, panel, mut background) in &mut panels {
        background.0 = panel_color(panel.hue, matches!(*interaction, Interaction::Hovered));
    }
}

fn panel_color(hue: f32, hovered: bool) -> Color {
    let boost = if hovered { 0.14 } else { 0.0 };
    Color::hsv(
        hue,
        (BASE_SATURATION + boost).min(1.0),
        (BASE_VALUE + boost).min(1.0),
    )
}

fn panel_hue(id: u64) -> f32 {
    // A tiny integer hash gives every new panel an unrelated-looking hue
    // without pulling a random-number dependency into this prototype.
    let mut value = id.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((value ^ (value >> 31)) % 360) as f32
}
