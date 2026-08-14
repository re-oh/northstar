//! Bevy rendering and input adapter for [`northstar_editor_ui`].
//!
//! The authoritative workspace is [`EditorUiModel`]. Bevy entities are
//! replaceable presentation objects keyed by Northstar-owned IDs.

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
pub use northstar_editor_ui::{
    EditorUiModel, LayoutSnapshot, ModelError, Panel, PanelId, Rect, SplitAxis, SplitId, Tab,
    TabId, TabRole, WidgetId,
};

pub mod prelude {
    pub use crate::{
        EditorUiModel, EditorUiRequest, EditorUiSet, EditorUiTheme, EditorUiWorkspace,
        NorthstarEditorUiBevyPlugin, PanelGeometryPoint, PanelId, PanelNode, PanelRoot,
        PanelTabBody, PanelTabNode, Rect, SplitAxis, SplitId, TabId, TabRole, WidgetId,
    };
}

#[derive(Resource, Clone, Debug)]
pub struct EditorUiTheme {
    pub background: Color,
    pub panel: Color,
    pub border: Color,
    pub tab: Color,
    pub tab_active: Color,
    pub text: Color,
}

impl Default for EditorUiTheme {
    fn default() -> Self {
        Self {
            background: Color::srgb(0.035, 0.04, 0.055),
            panel: Color::srgb(0.075, 0.085, 0.11),
            border: Color::srgb(0.19, 0.21, 0.27),
            tab: Color::srgb(0.12, 0.14, 0.18),
            tab_active: Color::srgb(0.22, 0.30, 0.46),
            text: Color::srgb(0.93, 0.94, 0.98),
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct EditorUiBevySettings {
    pub keyboard_shortcuts: bool,
    pub title_bar_height: f32,
    pub handle_radius: f32,
    pub edge_grab_distance: f32,
}

impl Default for EditorUiBevySettings {
    fn default() -> Self {
        Self {
            keyboard_shortcuts: true,
            title_bar_height: 30.0,
            handle_radius: 10.0,
            edge_grab_distance: 7.0,
        }
    }
}

#[derive(Default)]
pub struct NorthstarEditorUiBevyPlugin {
    pub model: EditorUiModel,
    pub settings: EditorUiBevySettings,
}

impl NorthstarEditorUiBevyPlugin {
    pub fn with_model(mut self, model: EditorUiModel) -> Self {
        self.model = model;
        self
    }

    pub fn with_keyboard_shortcuts(mut self, enabled: bool) -> Self {
        self.settings.keyboard_shortcuts = enabled;
        self
    }
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorUiSet {
    Input,
    Mutate,
    Render,
}

impl Plugin for NorthstarEditorUiBevyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EditorUiWorkspace(self.model.clone()))
            .insert_resource(self.settings.clone())
            .init_resource::<EditorUiTheme>()
            .init_resource::<UiEntityMap>()
            .init_resource::<ActiveDrag>()
            .add_message::<EditorUiRequest>()
            .configure_sets(
                Update,
                (EditorUiSet::Input, EditorUiSet::Mutate, EditorUiSet::Render).chain(),
            )
            .add_systems(Startup, setup_root)
            .add_systems(
                Update,
                (
                    (keyboard_requests, clicked_tab_requests, drag_splits)
                        .in_set(EditorUiSet::Input),
                    apply_requests.in_set(EditorUiSet::Mutate),
                    sync_model_to_bevy.in_set(EditorUiSet::Render),
                ),
            );
    }
}

#[derive(Message, Clone, Debug)]
pub enum EditorUiRequest {
    Split(PanelId),
    Remove(PanelId),
    ResizeSplit {
        split: SplitId,
        delta: f32,
    },
    AddTab {
        panel: PanelId,
        title: String,
        role: TabRole,
    },
    MoveTab {
        tab: TabId,
        to_panel: PanelId,
    },
    ActivateTab(TabId),
}

#[derive(Resource, Clone, Debug)]
pub struct EditorUiWorkspace(pub EditorUiModel);

#[derive(Component, Clone, Copy, Debug)]
pub struct PanelRoot;

#[derive(Component, Clone, Copy, Debug)]
pub struct PanelNode(pub PanelId);

#[derive(Component, Clone, Copy, Debug)]
pub struct PanelTabNode(pub TabId);

/// Application-owned children belong under this entity. It is reparented,
/// never recreated, when its tab moves between panels.
#[derive(Component, Clone, Copy, Debug)]
pub struct PanelTabBody(pub TabId);

#[derive(Component, Clone, Copy, Debug)]
pub struct PanelGeometryPoint {
    pub position: Vec2,
    pub is_intersection: bool,
    pub is_edge_midpoint: bool,
}

#[derive(Clone, Copy)]
struct PanelEntities {
    panel: Entity,
    tab_bar: Entity,
    content: Entity,
}

#[derive(Clone, Copy)]
struct TabEntities {
    tab: Entity,
    body: Entity,
    label: Entity,
}

#[derive(Resource, Default)]
struct UiEntityMap {
    root: Option<Entity>,
    panels: BTreeMap<PanelId, PanelEntities>,
    tabs: BTreeMap<TabId, TabEntities>,
}

#[derive(Resource, Default)]
struct ActiveDrag(Option<DragState>);

struct DragState {
    vertical: Option<SplitId>,
    horizontal: Option<SplitId>,
    start_cursor: Vec2,
    initial_model: EditorUiModel,
}

fn setup_root(
    mut commands: Commands,
    mut entities: ResMut<UiEntityMap>,
    theme: Res<EditorUiTheme>,
) {
    entities.root = Some(
        commands
            .spawn((
                PanelRoot,
                Node {
                    width: percent(100),
                    height: percent(100),
                    position_type: PositionType::Relative,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(theme.background),
            ))
            .id(),
    );
}

fn keyboard_requests(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<EditorUiBevySettings>,
    panels: Query<(&PanelNode, &Interaction)>,
    tabs: Query<(&PanelTabNode, &Interaction)>,
    workspace: Res<EditorUiWorkspace>,
    mut requests: MessageWriter<EditorUiRequest>,
) {
    if !settings.keyboard_shortcuts {
        return;
    }
    let hovered = panels
        .iter()
        .find(|(_, interaction)| is_hovered(interaction))
        .map(|(panel, _)| panel.0)
        .or_else(|| {
            tabs.iter()
                .find(|(_, interaction)| is_hovered(interaction))
                .and_then(|(tab, _)| owner_of_tab(&workspace.0, tab.0))
        });
    let Some(panel) = hovered else {
        return;
    };
    if keys.just_pressed(KeyCode::KeyQ) {
        requests.write(EditorUiRequest::Split(panel));
    }
    if keys.just_pressed(KeyCode::KeyE) {
        requests.write(EditorUiRequest::Remove(panel));
    }
}

fn clicked_tab_requests(
    tabs: Query<(&PanelTabNode, &Interaction), Changed<Interaction>>,
    mut requests: MessageWriter<EditorUiRequest>,
) {
    for (tab, interaction) in &tabs {
        if *interaction == Interaction::Pressed {
            requests.write(EditorUiRequest::ActivateTab(tab.0));
        }
    }
}

fn apply_requests(
    mut requests: MessageReader<EditorUiRequest>,
    mut workspace: ResMut<EditorUiWorkspace>,
) {
    let model = &mut workspace.0;
    for request in requests.read() {
        let result = match request {
            EditorUiRequest::Split(panel) => model.split_panel(*panel).map(|_| ()),
            EditorUiRequest::Remove(panel) => model.remove_panel(*panel),
            EditorUiRequest::ResizeSplit { split, delta } => model.resize_split(*split, *delta),
            EditorUiRequest::AddTab { panel, title, role } => {
                model.add_tab(*panel, title, *role).map(|_| ())
            }
            EditorUiRequest::MoveTab { tab, to_panel } => model.move_tab(*tab, *to_panel),
            EditorUiRequest::ActivateTab(tab) => model.activate_tab(*tab),
        };
        if let Err(error) = result {
            tracing::debug!(%error, "editor UI request was not applicable");
        }
    }
}

fn drag_splits(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    settings: Res<EditorUiBevySettings>,
    mut active: ResMut<ActiveDrag>,
    mut workspace: ResMut<EditorUiWorkspace>,
) {
    if mouse.just_released(MouseButton::Left) {
        active.0 = None;
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pixels) = window.cursor_position() else {
        return;
    };
    let size = Vec2::new(window.width(), window.height());
    let cursor = cursor_pixels / size;

    if mouse.just_pressed(MouseButton::Left) {
        let snapshot = workspace.0.snapshot();
        let (vertical, horizontal) = pick_splits(&snapshot, cursor_pixels, size, &settings);
        if vertical.is_some() || horizontal.is_some() {
            active.0 = Some(DragState {
                vertical,
                horizontal,
                start_cursor: cursor,
                initial_model: workspace.0.clone(),
            });
        }
    }

    let Some(drag) = &active.0 else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        return;
    }
    let delta = cursor - drag.start_cursor;
    workspace.0 = drag.initial_model.clone();
    if let Some(split) = drag.vertical {
        let _ = workspace.0.resize_split(split, delta.x);
    }
    if let Some(split) = drag.horizontal {
        let _ = workspace.0.resize_split(split, delta.y);
    }
}

fn pick_splits(
    snapshot: &LayoutSnapshot,
    cursor_pixels: Vec2,
    size: Vec2,
    settings: &EditorUiBevySettings,
) -> (Option<SplitId>, Option<SplitId>) {
    let cursor = cursor_pixels / size;
    let vertical = snapshot
        .splits
        .iter()
        .filter(|split| split.axis == SplitAxis::Vertical)
        .filter(|split| cursor.y >= split.bounds.y && cursor.y <= split.bounds.bottom())
        .map(|split| ((cursor.x - split.position).abs() * size.x, split.id))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .filter(|(distance, _)| *distance <= settings.edge_grab_distance)
        .map(|(_, id)| id);
    let horizontal = snapshot
        .splits
        .iter()
        .filter(|split| split.axis == SplitAxis::Horizontal)
        .filter(|split| cursor.x >= split.bounds.x && cursor.x <= split.bounds.right())
        .map(|split| ((cursor.y - split.position).abs() * size.y, split.id))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .filter(|(distance, _)| *distance <= settings.edge_grab_distance)
        .map(|(_, id)| id);
    let at_intersection = vertical.is_some()
        && horizontal.is_some()
        && snapshot.splits.iter().any(|split| {
            let distance = match split.axis {
                SplitAxis::Vertical => (cursor.x - split.position).abs() * size.x,
                SplitAxis::Horizontal => (cursor.y - split.position).abs() * size.y,
            };
            distance <= settings.handle_radius
        });
    if at_intersection {
        (vertical, horizontal)
    } else {
        let vertical_distance = vertical.and_then(|id| {
            snapshot
                .splits
                .iter()
                .find(|split| split.id == id)
                .map(|split| (cursor.x - split.position).abs() * size.x)
        });
        let horizontal_distance = horizontal.and_then(|id| {
            snapshot
                .splits
                .iter()
                .find(|split| split.id == id)
                .map(|split| (cursor.y - split.position).abs() * size.y)
        });
        match (vertical_distance, horizontal_distance) {
            (Some(v), Some(h)) if v <= h => (vertical, None),
            (Some(_), Some(_)) => (None, horizontal),
            _ => (vertical, horizontal),
        }
    }
}

fn sync_model_to_bevy(world: &mut World) {
    let model = world.resource::<EditorUiWorkspace>().0.clone();
    let snapshot = model.snapshot();
    let theme = world.resource::<EditorUiTheme>().clone();
    let settings = world.resource::<EditorUiBevySettings>().clone();
    let mut map = world.remove_resource::<UiEntityMap>().unwrap_or_default();
    let Some(root) = map.root else {
        world.insert_resource(map);
        return;
    };

    let live_tabs: Vec<TabId> = model.tabs().map(|tab| tab.id).collect();
    let removed_tabs: Vec<TabId> = map
        .tabs
        .keys()
        .filter(|id| !live_tabs.contains(id))
        .copied()
        .collect();
    for id in removed_tabs {
        if let Some(entities) = map.tabs.remove(&id) {
            if world.get_entity(entities.tab).is_ok() {
                world.despawn(entities.tab);
            }
            if world.get_entity(entities.body).is_ok() {
                world.despawn(entities.body);
            }
        }
    }

    let live_panels: Vec<PanelId> = model.panels().map(|panel| panel.id).collect();
    let removed_panels: Vec<PanelId> = map
        .panels
        .keys()
        .filter(|id| !live_panels.contains(id))
        .copied()
        .collect();
    for id in removed_panels {
        if let Some(entities) = map.panels.remove(&id)
            && world.get_entity(entities.panel).is_ok()
        {
            world.despawn(entities.panel);
        }
    }

    for panel_snapshot in &snapshot.panels {
        let entities = if let Some(entities) = map.panels.get(&panel_snapshot.id).copied() {
            entities
        } else {
            let entities = spawn_panel_entities(world, root, panel_snapshot.id, &theme, &settings);
            map.panels.insert(panel_snapshot.id, entities);
            entities
        };
        if let Some(mut node) = world.get_mut::<Node>(entities.panel) {
            apply_bounds(&mut node, panel_snapshot.bounds);
        }
    }

    for tab in model.tabs() {
        let entities = if let Some(entities) = map.tabs.get(&tab.id).copied() {
            entities
        } else {
            let entities = spawn_tab_entities(world, tab, &theme);
            map.tabs.insert(tab.id, entities);
            entities
        };
        if let Some(mut text) = world.get_mut::<Text>(entities.label) {
            **text = tab.title.clone();
        }
        let Some(owner) = owner_of_tab(&model, tab.id) else {
            continue;
        };
        let panel_entities = map.panels[&owner];
        world
            .entity_mut(panel_entities.tab_bar)
            .add_child(entities.tab);
        world
            .entity_mut(panel_entities.content)
            .add_child(entities.body);
        let active = model.panel(owner).and_then(|panel| panel.active_tab) == Some(tab.id);
        if let Some(mut node) = world.get_mut::<Node>(entities.body) {
            node.display = if active { Display::Flex } else { Display::None };
        }
        if let Some(mut color) = world.get_mut::<BackgroundColor>(entities.tab) {
            color.0 = if active { theme.tab_active } else { theme.tab };
        }
    }

    rebuild_geometry_points(world, &snapshot);
    world.insert_resource(map);
}

fn spawn_panel_entities(
    world: &mut World,
    root: Entity,
    id: PanelId,
    theme: &EditorUiTheme,
    settings: &EditorUiBevySettings,
) -> PanelEntities {
    let panel = world
        .spawn((
            PanelNode(id),
            Interaction::None,
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(panel_hue(id)),
            BorderColor::all(theme.border),
        ))
        .id();
    let tab_bar = world
        .spawn((
            Node {
                width: percent(100),
                height: px(settings.title_bar_height),
                min_height: px(settings.title_bar_height),
                column_gap: px(2),
                padding: UiRect::horizontal(px(4)),
                ..default()
            },
            BackgroundColor(theme.panel),
        ))
        .id();
    let content = world
        .spawn(Node {
            width: percent(100),
            flex_grow: 1.0,
            position_type: PositionType::Relative,
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    world.entity_mut(panel).add_children(&[tab_bar, content]);
    world.entity_mut(root).add_child(panel);
    PanelEntities {
        panel,
        tab_bar,
        content,
    }
}

fn spawn_tab_entities(world: &mut World, tab: &Tab, theme: &EditorUiTheme) -> TabEntities {
    let body = world
        .spawn((
            PanelTabBody(tab.id),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: if tab.role == TabRole::Toolbar {
                    FlexDirection::Row
                } else {
                    FlexDirection::Column
                },
                ..default()
            },
        ))
        .id();
    let tab_entity = world
        .spawn((
            PanelTabNode(tab.id),
            Interaction::None,
            Node {
                height: percent(100),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(px(8)),
                ..default()
            },
            BackgroundColor(theme.tab),
        ))
        .id();
    let label = world
        .spawn((
            Text::new(tab.title.clone()),
            TextFont::from_font_size(15.0),
            TextColor(theme.text),
        ))
        .id();
    world.entity_mut(tab_entity).add_child(label);
    TabEntities {
        tab: tab_entity,
        body,
        label,
    }
}

fn rebuild_geometry_points(world: &mut World, snapshot: &LayoutSnapshot) {
    let old: Vec<Entity> = world
        .query_filtered::<Entity, With<PanelGeometryPoint>>()
        .iter(world)
        .collect();
    for entity in old {
        world.despawn(entity);
    }
    let mut points: HashMap<(i32, i32), PanelGeometryPoint> = HashMap::new();
    for panel in &snapshot.panels {
        let bounds = panel.bounds;
        let middle_x = (bounds.x + bounds.right()) * 0.5;
        let middle_y = (bounds.y + bounds.bottom()) * 0.5;
        for position in [
            Vec2::new(bounds.x, bounds.y),
            Vec2::new(bounds.right(), bounds.y),
            Vec2::new(bounds.x, bounds.bottom()),
            Vec2::new(bounds.right(), bounds.bottom()),
        ] {
            merge_point(&mut points, position, true, false);
        }
        for position in [
            Vec2::new(middle_x, bounds.y),
            Vec2::new(middle_x, bounds.bottom()),
            Vec2::new(bounds.x, middle_y),
            Vec2::new(bounds.right(), middle_y),
        ] {
            merge_point(&mut points, position, false, true);
        }
    }
    world.spawn_batch(points.into_values());
}

fn merge_point(
    points: &mut HashMap<(i32, i32), PanelGeometryPoint>,
    position: Vec2,
    intersection: bool,
    midpoint: bool,
) {
    let key = (
        (position.x * 1_000_000.0).round() as i32,
        (position.y * 1_000_000.0).round() as i32,
    );
    let point = points.entry(key).or_insert(PanelGeometryPoint {
        position,
        is_intersection: false,
        is_edge_midpoint: false,
    });
    point.is_intersection |= intersection;
    point.is_edge_midpoint |= midpoint;
}

fn apply_bounds(node: &mut Node, bounds: Rect) {
    node.left = percent(bounds.x * 100.0);
    node.top = percent(bounds.y * 100.0);
    node.width = percent(bounds.width * 100.0);
    node.height = percent(bounds.height * 100.0);
}

fn owner_of_tab(model: &EditorUiModel, tab: TabId) -> Option<PanelId> {
    model
        .panels()
        .find(|panel| panel.tabs.contains(&tab))
        .map(|panel| panel.id)
}

fn is_hovered(interaction: &Interaction) -> bool {
    matches!(*interaction, Interaction::Hovered | Interaction::Pressed)
}

fn panel_hue(id: PanelId) -> Color {
    let mut value = id.get().wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    Color::hsv(((value ^ (value >> 31)) % 360) as f32, 0.48, 0.62)
}
