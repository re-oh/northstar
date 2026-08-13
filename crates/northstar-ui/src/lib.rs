//! Tiled panels for Northstar tools and editor surfaces.
//!
//! Add [`NorthstarUiPlugin`] to an app that already has a UI camera. Panels
//! are ordinary ECS entities with [`PanelId`] and [`PanelBounds`]. Other
//! systems mutate the layout by writing [`PanelRequest`] messages.
//!
//! The default plugin enables the current development controls: hover a
//! panel and press `Q` to split it or `E` to remove it, and drag internal
//! edges/intersections with the left mouse button.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

const GEOMETRY_EPSILON: f32 = 0.000_01;

/// Common imports for applications embedding the panel SDK.
pub mod prelude {
    pub use crate::{
        NorthstarUiPlugin, NorthstarUiPrimitivesPlugin, PanelBounds, PanelContent,
        PanelGeometryPoint, PanelId, PanelRequest, PanelRoot, PanelShell, PanelTab, PanelTabBar,
        PanelTabBody, PanelTabKind, PanelTabOwner, PanelTitle, PanelUiSet, PanelUiSettings,
        UiButton, UiButtonBundle, UiButtonTone, UiStack, UiStackBundle, UiSurface, UiSurfaceBundle,
        UiTheme,
    };
}

/// Small visual token set shared by the launcher and early editor tools.
#[derive(Resource, Clone, Debug)]
pub struct UiTheme {
    pub background: Color,
    pub surface: Color,
    pub border: Color,
    pub text: Color,
    pub primary: Color,
    pub primary_hovered: Color,
    pub primary_pressed: Color,
    pub secondary: Color,
    pub secondary_hovered: Color,
    pub secondary_pressed: Color,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            background: Color::srgb(0.035, 0.04, 0.055),
            surface: Color::srgb(0.075, 0.085, 0.11),
            border: Color::srgb(0.19, 0.21, 0.27),
            text: Color::srgb(0.93, 0.94, 0.98),
            primary: Color::srgb(0.20, 0.38, 0.72),
            primary_hovered: Color::srgb(0.27, 0.48, 0.88),
            primary_pressed: Color::srgb(0.16, 0.31, 0.61),
            secondary: Color::srgb(0.14, 0.16, 0.21),
            secondary_hovered: Color::srgb(0.20, 0.23, 0.30),
            secondary_pressed: Color::srgb(0.10, 0.12, 0.16),
        }
    }
}

/// Installs reusable visual primitives without creating a panel workspace.
pub struct NorthstarUiPrimitivesPlugin;

impl Plugin for NorthstarUiPrimitivesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTheme>()
            .add_systems(Update, update_ui_button_visuals);
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct UiSurface;

#[derive(Bundle)]
pub struct UiSurfaceBundle {
    pub surface: UiSurface,
    pub node: Node,
    pub background: BackgroundColor,
    pub border: BorderColor,
}

impl UiSurfaceBundle {
    pub fn full_screen(theme: &UiTheme) -> Self {
        Self {
            surface: UiSurface,
            node: Node {
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            background: BackgroundColor(theme.background),
            border: BorderColor::all(theme.border),
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct UiStack {
    pub direction: FlexDirection,
}

#[derive(Bundle)]
pub struct UiStackBundle {
    pub stack: UiStack,
    pub node: Node,
}

impl UiStackBundle {
    pub fn column(gap: f32) -> Self {
        Self::new(FlexDirection::Column, gap)
    }

    pub fn row(gap: f32) -> Self {
        Self::new(FlexDirection::Row, gap)
    }

    fn new(direction: FlexDirection, gap: f32) -> Self {
        Self {
            stack: UiStack { direction },
            node: Node {
                flex_direction: direction,
                row_gap: px(gap),
                column_gap: px(gap),
                ..default()
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiButtonTone {
    #[default]
    Primary,
    Secondary,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct UiButton {
    pub tone: UiButtonTone,
}

#[derive(Bundle)]
pub struct UiButtonBundle {
    pub button: Button,
    pub ui_button: UiButton,
    pub node: Node,
    pub background: BackgroundColor,
    pub border: BorderColor,
}

impl UiButtonBundle {
    pub fn new(tone: UiButtonTone, theme: &UiTheme) -> Self {
        Self {
            button: Button,
            ui_button: UiButton { tone },
            node: Node {
                width: px(260),
                height: px(48),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1)),
                ..default()
            },
            background: BackgroundColor(button_color(theme, tone, Interaction::None)),
            border: BorderColor::all(theme.border),
        }
    }
}

fn update_ui_button_visuals(
    theme: Res<UiTheme>,
    mut buttons: Query<(&Interaction, &UiButton, &mut BackgroundColor), Changed<Interaction>>,
) {
    for (interaction, button, mut background) in &mut buttons {
        background.0 = button_color(&theme, button.tone, *interaction);
    }
}

fn button_color(theme: &UiTheme, tone: UiButtonTone, interaction: Interaction) -> Color {
    match (tone, interaction) {
        (UiButtonTone::Primary, Interaction::Pressed) => theme.primary_pressed,
        (UiButtonTone::Primary, Interaction::Hovered) => theme.primary_hovered,
        (UiButtonTone::Primary, Interaction::None) => theme.primary,
        (UiButtonTone::Secondary, Interaction::Pressed) => theme.secondary_pressed,
        (UiButtonTone::Secondary, Interaction::Hovered) => theme.secondary_hovered,
        (UiButtonTone::Secondary, Interaction::None) => theme.secondary,
    }
}

/// Runtime tuning for panel appearance and direct manipulation.
#[derive(Resource, Clone, Debug)]
pub struct PanelUiSettings {
    pub title_bar_height: f32,
    pub base_saturation: f32,
    pub base_value: f32,
    pub hover_boost: f32,
    pub handle_radius: f32,
    pub edge_grab_distance: f32,
    /// Minimum normalized width and height of a panel.
    pub min_panel_size: f32,
}

impl Default for PanelUiSettings {
    fn default() -> Self {
        Self {
            title_bar_height: 30.0,
            base_saturation: 0.48,
            base_value: 0.62,
            hover_boost: 0.14,
            handle_radius: 10.0,
            edge_grab_distance: 7.0,
            min_panel_size: 0.08,
        }
    }
}

/// Installs the tiled-panel runtime.
pub struct NorthstarUiPlugin {
    pub settings: PanelUiSettings,
    /// Creates one full-screen panel during startup.
    pub spawn_initial_panel: bool,
    /// Enables the development Q/E shortcuts.
    pub keyboard_shortcuts: bool,
}

impl Default for NorthstarUiPlugin {
    fn default() -> Self {
        Self {
            settings: PanelUiSettings::default(),
            spawn_initial_panel: true,
            keyboard_shortcuts: true,
        }
    }
}

impl NorthstarUiPlugin {
    /// Builds a panel runtime without app-specific keyboard bindings.
    pub fn sdk() -> Self {
        Self {
            keyboard_shortcuts: false,
            ..Self::default()
        }
    }

    pub fn with_settings(mut self, settings: PanelUiSettings) -> Self {
        self.settings = settings;
        self
    }

    pub fn with_keyboard_shortcuts(mut self, enabled: bool) -> Self {
        self.keyboard_shortcuts = enabled;
        self
    }

    pub fn with_initial_panel(mut self, enabled: bool) -> Self {
        self.spawn_initial_panel = enabled;
        self
    }
}

#[derive(Resource)]
struct PanelUiOptions {
    spawn_initial_panel: bool,
    keyboard_shortcuts: bool,
}

/// Ordering hooks for applications integrating custom panel input or UI.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelUiSet {
    Input,
    Mutate,
    Visual,
}

impl Plugin for NorthstarUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<NorthstarUiPrimitivesPlugin>() {
            app.add_plugins(NorthstarUiPrimitivesPlugin);
        }
        app.insert_resource(self.settings.clone())
            .insert_resource(PanelUiOptions {
                spawn_initial_panel: self.spawn_initial_panel,
                keyboard_shortcuts: self.keyboard_shortcuts,
            })
            .init_resource::<PanelLayout>()
            .init_resource::<PanelDrag>()
            .add_message::<PanelRequest>()
            .configure_sets(
                Update,
                (PanelUiSet::Input, PanelUiSet::Mutate, PanelUiSet::Visual).chain(),
            )
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    keyboard_panel_requests.in_set(PanelUiSet::Input),
                    clicked_tab_requests.in_set(PanelUiSet::Input),
                    apply_panel_requests.in_set(PanelUiSet::Mutate),
                    drag_panel_geometry.in_set(PanelUiSet::Mutate),
                    update_panel_hover.in_set(PanelUiSet::Visual),
                ),
            );
    }
}

/// Stable identity for a panel in the current process.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PanelId(pub u64);

impl PanelId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// A panel's normalized bounds, measured from the top-left of the window.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PanelBounds {
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }
}

/// A deduplicated point in the panel-edge graph.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelGeometryPoint {
    pub position: Vec2,
    pub is_intersection: bool,
    pub is_edge_midpoint: bool,
}

/// Marker on the text entity used as a panel's title.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelTitle(pub PanelId);

/// Typed layout mutations accepted by the panel runtime.
#[derive(Message, Clone, Debug)]
pub enum PanelRequest {
    Split {
        panel: Entity,
    },
    Remove {
        panel: Entity,
    },
    AddTab {
        panel: Entity,
        title: String,
        kind: PanelTabKind,
    },
    MoveTab {
        tab: Entity,
        to_panel: Entity,
    },
    ActivateTab {
        tab: Entity,
    },
}

impl PanelRequest {
    pub fn split(panel: Entity) -> Self {
        Self::Split { panel }
    }

    pub fn remove(panel: Entity) -> Self {
        Self::Remove { panel }
    }

    pub fn add_tab(panel: Entity, title: impl Into<String>, kind: PanelTabKind) -> Self {
        Self::AddTab {
            panel,
            title: title.into(),
            kind,
        }
    }

    pub fn move_tab(tab: Entity, to_panel: Entity) -> Self {
        Self::MoveTab { tab, to_panel }
    }

    pub fn activate_tab(tab: Entity) -> Self {
        Self::ActivateTab { tab }
    }
}

/// Root UI node containing every tiled panel.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelRoot;

/// Semantic role of a tab body. The body itself remains an ordinary Bevy UI
/// node and may contain any application-defined widgets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelTabKind {
    #[default]
    View,
    Inspector,
    Toolbar,
}

/// A panel's reusable tab/content shell.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelShell {
    pub tab_bar: Entity,
    pub content: Entity,
    pub active_tab: Option<Entity>,
}

/// A movable tab. Add application UI as children of [`PanelTab::body`].
#[derive(Component, Clone, Debug)]
pub struct PanelTab {
    pub title: String,
    pub kind: PanelTabKind,
    pub body: Entity,
}

/// Identifies the panel currently hosting a tab.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelTabOwner(pub Entity);

/// Marker for a panel's tab-strip node.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelTabBar;

/// Marker for a panel's active-content container.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelContent;

/// Marker for a tab's application-owned body node.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelTabBody;

#[derive(Component)]
struct PanelColor {
    hue: f32,
}

#[derive(Resource, Default)]
struct PanelLayout {
    next_id: u64,
}

#[derive(Resource, Default)]
struct PanelDrag {
    active: Option<ActiveDrag>,
}

struct ActiveDrag {
    handle: DragHandle,
    start_cursor: Vec2,
    initial_bounds: Vec<(Entity, PanelBounds)>,
}

type ChangedPanelTabs = (With<PanelTab>, Changed<Interaction>);

#[derive(Clone, Copy)]
enum DragHandle {
    Point(Vec2),
    Vertical(f32),
    Horizontal(f32),
}

fn setup(
    mut commands: Commands,
    mut layout: ResMut<PanelLayout>,
    options: Res<PanelUiOptions>,
    settings: Res<PanelUiSettings>,
) {
    let root = commands
        .spawn((
            PanelRoot,
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

    if options.spawn_initial_panel {
        spawn_panel(
            &mut commands,
            root,
            &mut layout,
            PanelBounds::FULL,
            &settings,
        );
    }
}

fn keyboard_panel_requests(
    keys: Res<ButtonInput<KeyCode>>,
    options: Res<PanelUiOptions>,
    panels: Query<(Entity, &Interaction), With<PanelId>>,
    tabs: Query<(&Interaction, &PanelTabOwner), With<PanelTab>>,
    mut requests: MessageWriter<PanelRequest>,
) {
    if !options.keyboard_shortcuts {
        return;
    }

    let panel = panels
        .iter()
        .find(|(_, interaction)| {
            matches!(**interaction, Interaction::Hovered | Interaction::Pressed)
        })
        .map(|(panel, _)| panel)
        .or_else(|| {
            tabs.iter()
                .find(|(interaction, _)| {
                    matches!(**interaction, Interaction::Hovered | Interaction::Pressed)
                })
                .map(|(_, owner)| owner.0)
        });
    let Some(panel) = panel else {
        return;
    };

    if keys.just_pressed(KeyCode::KeyQ) {
        requests.write(PanelRequest::Split { panel });
    }
    if keys.just_pressed(KeyCode::KeyE) {
        requests.write(PanelRequest::Remove { panel });
    }
}

fn clicked_tab_requests(
    tabs: Query<(Entity, &Interaction), ChangedPanelTabs>,
    mut requests: MessageWriter<PanelRequest>,
) {
    for (tab, interaction) in &tabs {
        if *interaction == Interaction::Pressed {
            requests.write(PanelRequest::ActivateTab { tab });
        }
    }
}

fn apply_panel_requests(
    mut requests: MessageReader<PanelRequest>,
    mut commands: Commands,
    root: Single<Entity, With<PanelRoot>>,
    mut layout: ResMut<PanelLayout>,
    settings: Res<PanelUiSettings>,
    mut panels: Query<(&mut PanelBounds, &mut Node), With<PanelId>>,
) {
    for request in requests.read() {
        match request {
            PanelRequest::Split { panel } => {
                let Ok((mut bounds, mut node)) = panels.get_mut(*panel) else {
                    continue;
                };
                let (existing_bounds, new_bounds) = split_bounds(*bounds);
                *bounds = existing_bounds;
                apply_bounds(&mut node, existing_bounds);
                spawn_panel(&mut commands, *root, &mut layout, new_bounds, &settings);
            }
            PanelRequest::Remove { panel } => {
                if panels.iter().count() > 1 {
                    let panel = *panel;
                    commands.queue(move |world: &mut World| {
                        remove_panel_and_fill_gap(world, panel);
                    });
                }
            }
            PanelRequest::AddTab { panel, title, kind } => {
                let (panel, title, kind) = (*panel, title.clone(), *kind);
                commands.queue(move |world: &mut World| {
                    spawn_panel_tab(world, panel, title, kind);
                });
            }
            PanelRequest::MoveTab { tab, to_panel } => {
                let (tab, to_panel) = (*tab, *to_panel);
                commands.queue(move |world: &mut World| move_panel_tab(world, tab, to_panel));
            }
            PanelRequest::ActivateTab { tab } => {
                let tab = *tab;
                commands.queue(move |world: &mut World| activate_panel_tab(world, tab));
            }
        }
    }
}

fn drag_panel_geometry(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut drag: ResMut<PanelDrag>,
    geometry_points: Query<&PanelGeometryPoint>,
    mut panels: Query<(Entity, &mut PanelBounds, &mut Node), With<PanelId>>,
    mut commands: Commands,
    settings: Res<PanelUiSettings>,
) {
    if mouse.just_released(MouseButton::Left) {
        drag.active = None;
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pixels) = window.cursor_position() else {
        return;
    };
    let window_size = Vec2::new(window.width(), window.height());
    if window_size.x <= 0.0 || window_size.y <= 0.0 {
        return;
    }
    let cursor = cursor_pixels / window_size;

    if mouse.just_pressed(MouseButton::Left) {
        let bounds: Vec<(Entity, PanelBounds)> = panels
            .iter_mut()
            .map(|(entity, bounds, _)| (entity, *bounds))
            .collect();
        if let Some(handle) = pick_drag_handle(
            cursor_pixels,
            window_size,
            &geometry_points,
            &bounds,
            &settings,
        ) {
            drag.active = Some(ActiveDrag {
                handle,
                start_cursor: cursor,
                initial_bounds: bounds,
            });
        }
    }

    let Some(active) = &drag.active else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        return;
    }

    let requested_delta = cursor - active.start_cursor;
    let delta = clamp_drag_delta(
        active.handle,
        requested_delta,
        &active.initial_bounds,
        settings.min_panel_size,
    );

    for (entity, mut bounds, mut node) in &mut panels {
        let Some((_, initial)) = active
            .initial_bounds
            .iter()
            .find(|(initial_entity, _)| *initial_entity == entity)
        else {
            continue;
        };
        let mut resized = *initial;
        match active.handle {
            DragHandle::Point(point) => {
                move_vertical_boundary(&mut resized, point.x, delta.x);
                move_horizontal_boundary(&mut resized, point.y, delta.y);
            }
            DragHandle::Vertical(x) => move_vertical_boundary(&mut resized, x, delta.x),
            DragHandle::Horizontal(y) => move_horizontal_boundary(&mut resized, y, delta.y),
        }
        *bounds = resized;
        apply_bounds(&mut node, resized);
    }

    commands.queue(|world: &mut World| rebuild_geometry(world));
}

fn pick_drag_handle(
    cursor_pixels: Vec2,
    window_size: Vec2,
    geometry_points: &Query<&PanelGeometryPoint>,
    panels: &[(Entity, PanelBounds)],
    settings: &PanelUiSettings,
) -> Option<DragHandle> {
    let point = geometry_points
        .iter()
        .filter(|point| point.is_intersection)
        .filter(|point| {
            let movable_x = point.position.x > 0.0 && point.position.x < 1.0;
            let movable_y = point.position.y > 0.0 && point.position.y < 1.0;
            movable_x || movable_y
        })
        .min_by(|a, b| {
            (a.position * window_size)
                .distance_squared(cursor_pixels)
                .total_cmp(&(b.position * window_size).distance_squared(cursor_pixels))
        })
        .filter(|point| {
            (point.position * window_size).distance(cursor_pixels) <= settings.handle_radius
        });
    if let Some(point) = point {
        return Some(DragHandle::Point(point.position));
    }

    let cursor = cursor_pixels / window_size;
    let mut best: Option<(f32, DragHandle)> = None;
    for (_, bounds) in panels {
        let left = bounds.x;
        let right = bounds.x + bounds.width;
        let top = bounds.y;
        let bottom = bounds.y + bounds.height;

        if cursor.y >= top && cursor.y <= bottom {
            for x in [left, right] {
                if x <= 0.0 || x >= 1.0 {
                    continue;
                }
                keep_nearest_handle(
                    &mut best,
                    (cursor.x - x).abs() * window_size.x,
                    DragHandle::Vertical(x),
                );
            }
        }
        if cursor.x >= left && cursor.x <= right {
            for y in [top, bottom] {
                if y <= 0.0 || y >= 1.0 {
                    continue;
                }
                keep_nearest_handle(
                    &mut best,
                    (cursor.y - y).abs() * window_size.y,
                    DragHandle::Horizontal(y),
                );
            }
        }
    }

    best.filter(|(distance, _)| *distance <= settings.edge_grab_distance)
        .map(|(_, handle)| handle)
}

fn keep_nearest_handle(best: &mut Option<(f32, DragHandle)>, distance: f32, handle: DragHandle) {
    if best
        .as_ref()
        .is_none_or(|(best_distance, _)| distance < *best_distance)
    {
        *best = Some((distance, handle));
    }
}

fn clamp_drag_delta(
    handle: DragHandle,
    requested: Vec2,
    panels: &[(Entity, PanelBounds)],
    min_panel_size: f32,
) -> Vec2 {
    let (vertical, horizontal) = match handle {
        DragHandle::Point(point) => (
            (point.x > 0.0 && point.x < 1.0).then_some(point.x),
            (point.y > 0.0 && point.y < 1.0).then_some(point.y),
        ),
        DragHandle::Vertical(x) => (Some(x), None),
        DragHandle::Horizontal(y) => (None, Some(y)),
    };
    Vec2::new(
        vertical.map_or(0.0, |x| {
            clamp_vertical_delta(x, requested.x, panels, min_panel_size)
        }),
        horizontal.map_or(0.0, |y| {
            clamp_horizontal_delta(y, requested.y, panels, min_panel_size)
        }),
    )
}

fn clamp_vertical_delta(
    x: f32,
    requested: f32,
    panels: &[(Entity, PanelBounds)],
    min_panel_size: f32,
) -> f32 {
    let mut minimum = f32::NEG_INFINITY;
    let mut maximum = f32::INFINITY;
    for (_, bounds) in panels {
        if nearly_equal(bounds.x + bounds.width, x) {
            minimum = minimum.max(min_panel_size - bounds.width);
        }
        if nearly_equal(bounds.x, x) {
            maximum = maximum.min(bounds.width - min_panel_size);
        }
    }
    requested.clamp(minimum, maximum)
}

fn clamp_horizontal_delta(
    y: f32,
    requested: f32,
    panels: &[(Entity, PanelBounds)],
    min_panel_size: f32,
) -> f32 {
    let mut minimum = f32::NEG_INFINITY;
    let mut maximum = f32::INFINITY;
    for (_, bounds) in panels {
        if nearly_equal(bounds.y + bounds.height, y) {
            minimum = minimum.max(min_panel_size - bounds.height);
        }
        if nearly_equal(bounds.y, y) {
            maximum = maximum.min(bounds.height - min_panel_size);
        }
    }
    requested.clamp(minimum, maximum)
}

fn move_vertical_boundary(bounds: &mut PanelBounds, x: f32, delta: f32) {
    if nearly_equal(bounds.x + bounds.width, x) {
        bounds.width += delta;
    }
    if nearly_equal(bounds.x, x) {
        bounds.x += delta;
        bounds.width -= delta;
    }
}

fn move_horizontal_boundary(bounds: &mut PanelBounds, y: f32, delta: f32) {
    if nearly_equal(bounds.y + bounds.height, y) {
        bounds.height += delta;
    }
    if nearly_equal(bounds.y, y) {
        bounds.y += delta;
        bounds.height -= delta;
    }
}

fn nearly_equal(a: f32, b: f32) -> bool {
    (a - b).abs() <= GEOMETRY_EPSILON
}

fn spawn_panel(
    commands: &mut Commands,
    root: Entity,
    layout: &mut PanelLayout,
    bounds: PanelBounds,
    settings: &PanelUiSettings,
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
            BackgroundColor(panel_color(hue, false, settings)),
        ))
        .id();
    let tab_bar = commands
        .spawn((
            PanelTabBar,
            Node {
                width: percent(100),
                height: px(settings.title_bar_height),
                min_height: px(settings.title_bar_height),
                align_items: AlignItems::Center,
                column_gap: px(2),
                padding: UiRect::horizontal(px(4)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.24)),
        ))
        .id();
    let content = commands
        .spawn((
            PanelContent,
            Node {
                width: percent(100),
                flex_grow: 1.0,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(panel).insert(PanelShell {
        tab_bar,
        content,
        active_tab: None,
    });
    commands.entity(panel).add_children(&[tab_bar, content]);
    commands.entity(root).add_child(panel);

    commands.queue(move |world: &mut World| {
        spawn_panel_tab(world, panel, format!("Panel {}", id.0), PanelTabKind::View);
        rebuild_geometry(world);
    });
}

fn spawn_panel_tab(
    world: &mut World,
    panel: Entity,
    title: String,
    kind: PanelTabKind,
) -> Option<Entity> {
    let shell = *world.get::<PanelShell>(panel)?;
    let panel_id = *world.get::<PanelId>(panel)?;
    let body = world
        .spawn((
            PanelTabBody,
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: if kind == PanelTabKind::Toolbar {
                    FlexDirection::Row
                } else {
                    FlexDirection::Column
                },
                display: Display::None,
                ..default()
            },
        ))
        .id();
    let tab = world
        .spawn((
            PanelTab {
                title: title.clone(),
                kind,
                body,
            },
            PanelTabOwner(panel),
            Interaction::None,
            Node {
                height: percent(100),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(px(8)),
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.16)),
        ))
        .id();
    let label = world
        .spawn((
            PanelTitle(panel_id),
            Text::new(title),
            TextFont::from_font_size(15.0),
            TextColor(Color::WHITE),
        ))
        .id();
    world.entity_mut(tab).add_child(label);
    world.entity_mut(shell.tab_bar).add_child(tab);
    world.entity_mut(shell.content).add_child(body);

    if shell.active_tab.is_none() {
        activate_panel_tab(world, tab);
    }
    Some(tab)
}

fn activate_panel_tab(world: &mut World, tab: Entity) {
    let Some(owner) = world.get::<PanelTabOwner>(tab).copied() else {
        return;
    };
    let tabs: Vec<(Entity, Entity)> = world
        .query::<(Entity, &PanelTab, &PanelTabOwner)>()
        .iter(world)
        .filter(|(_, _, tab_owner)| tab_owner.0 == owner.0)
        .map(|(entity, panel_tab, _)| (entity, panel_tab.body))
        .collect();
    if !tabs.iter().any(|(entity, _)| *entity == tab) {
        return;
    }

    for (entity, body) in tabs {
        if let Some(mut node) = world.get_mut::<Node>(body) {
            node.display = if entity == tab {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    if let Some(mut shell) = world.get_mut::<PanelShell>(owner.0) {
        shell.active_tab = Some(tab);
    }
}

fn move_panel_tab(world: &mut World, tab: Entity, to_panel: Entity) {
    let Some(from_panel) = world
        .get::<PanelTabOwner>(tab)
        .copied()
        .map(|owner| owner.0)
    else {
        return;
    };
    let Some(tab_body) = world.get::<PanelTab>(tab).map(|panel_tab| panel_tab.body) else {
        return;
    };
    let Some(destination) = world.get::<PanelShell>(to_panel).copied() else {
        return;
    };
    if from_panel == to_panel {
        activate_panel_tab(world, tab);
        return;
    }

    let was_active = world
        .get::<PanelShell>(from_panel)
        .is_some_and(|shell| shell.active_tab == Some(tab));
    world.entity_mut(destination.tab_bar).add_child(tab);
    world.entity_mut(destination.content).add_child(tab_body);
    world.entity_mut(tab).insert(PanelTabOwner(to_panel));

    if was_active {
        let replacement = world
            .query::<(Entity, &PanelTabOwner)>()
            .iter(world)
            .find(|(entity, owner)| owner.0 == from_panel && *entity != tab)
            .map(|(entity, _)| entity);
        if let Some(replacement) = replacement {
            activate_panel_tab(world, replacement);
        } else if let Some(mut shell) = world.get_mut::<PanelShell>(from_panel) {
            shell.active_tab = None;
        }
    }
    activate_panel_tab(world, tab);
}

#[derive(Clone, Copy)]
enum MergeSide {
    Left,
    Right,
    Top,
    Bottom,
}

fn remove_panel_and_fill_gap(world: &mut World, removed_entity: Entity) {
    let Some(removed) = world.get::<PanelBounds>(removed_entity).copied() else {
        return;
    };
    let panels: Vec<(Entity, PanelBounds)> = world
        .query::<(Entity, &PanelBounds)>()
        .iter(world)
        .filter(|(entity, _)| *entity != removed_entity)
        .map(|(entity, bounds)| (entity, *bounds))
        .collect();

    let Some((side, neighbors)) = [
        MergeSide::Left,
        MergeSide::Right,
        MergeSide::Top,
        MergeSide::Bottom,
    ]
    .into_iter()
    .filter_map(|side| {
        let neighbors = neighbors_covering_side(side, removed, &panels);
        (!neighbors.is_empty()).then_some((side, neighbors))
    })
    .min_by_key(|(_, neighbors)| neighbors.len()) else {
        // Keeping the panel is safer than destroying the user's layout if a
        // future non-rectangular topology cannot be filled by local neighbors.
        return;
    };

    for entity in neighbors {
        let panel = world.entity_mut(entity);
        let mut bounds = *panel
            .get::<PanelBounds>()
            .expect("merge neighbors are panels");
        match side {
            MergeSide::Left => bounds.width += removed.width,
            MergeSide::Right => {
                bounds.x = removed.x;
                bounds.width += removed.width;
            }
            MergeSide::Top => bounds.height += removed.height,
            MergeSide::Bottom => {
                bounds.y = removed.y;
                bounds.height += removed.height;
            }
        }
        let mut panel = world.entity_mut(entity);
        panel.insert(bounds);
        let mut node = panel.get_mut::<Node>().expect("panels always have a Node");
        apply_bounds(&mut node, bounds);
    }

    world.despawn(removed_entity);
    world.resource_mut::<PanelDrag>().active = None;
    rebuild_geometry(world);
}

fn neighbors_covering_side(
    side: MergeSide,
    removed: PanelBounds,
    panels: &[(Entity, PanelBounds)],
) -> Vec<Entity> {
    let removed_right = removed.x + removed.width;
    let removed_bottom = removed.y + removed.height;
    let mut intervals: Vec<(f32, f32, Entity)> = panels
        .iter()
        .filter_map(|(entity, bounds)| {
            let right = bounds.x + bounds.width;
            let bottom = bounds.y + bounds.height;
            let (touches, start, end, target_start, target_end) = match side {
                MergeSide::Left => (
                    nearly_equal(right, removed.x),
                    bounds.y,
                    bottom,
                    removed.y,
                    removed_bottom,
                ),
                MergeSide::Right => (
                    nearly_equal(bounds.x, removed_right),
                    bounds.y,
                    bottom,
                    removed.y,
                    removed_bottom,
                ),
                MergeSide::Top => (
                    nearly_equal(bottom, removed.y),
                    bounds.x,
                    right,
                    removed.x,
                    removed_right,
                ),
                MergeSide::Bottom => (
                    nearly_equal(bounds.y, removed_bottom),
                    bounds.x,
                    right,
                    removed.x,
                    removed_right,
                ),
            };
            (touches
                && start >= target_start - GEOMETRY_EPSILON
                && end <= target_end + GEOMETRY_EPSILON)
                .then_some((start, end, *entity))
        })
        .collect();
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));

    let (target_start, target_end) = match side {
        MergeSide::Left | MergeSide::Right => (removed.y, removed_bottom),
        MergeSide::Top | MergeSide::Bottom => (removed.x, removed_right),
    };
    let mut covered_until = target_start;
    for (start, end, _) in &intervals {
        if *start > covered_until + GEOMETRY_EPSILON {
            return Vec::new();
        }
        covered_until = covered_until.max(*end);
    }
    if covered_until < target_end - GEOMETRY_EPSILON {
        return Vec::new();
    }

    intervals.into_iter().map(|(_, _, entity)| entity).collect()
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
    settings: Res<PanelUiSettings>,
) {
    for (interaction, panel, mut background) in &mut panels {
        background.0 = panel_color(
            panel.hue,
            matches!(*interaction, Interaction::Hovered),
            &settings,
        );
    }
}

fn panel_color(hue: f32, hovered: bool, settings: &PanelUiSettings) -> Color {
    let boost = if hovered { settings.hover_boost } else { 0.0 };
    Color::hsv(
        hue,
        (settings.base_saturation + boost).min(1.0),
        (settings.base_value + boost).min(1.0),
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
