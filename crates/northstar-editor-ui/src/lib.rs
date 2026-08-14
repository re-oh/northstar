//! Bevy-free authoritative model for Northstar editor UI workspaces.
//!
//! Renderers consume [`LayoutSnapshot`] values and map Northstar-owned IDs to
//! their own nodes. Serialized workspaces never contain renderer entities.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(u64);

        impl $name {
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(PanelId);
id_type!(TabId);
id_type!(WidgetId);
id_type!(SplitId);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

/// Visual role only. This deliberately does not duplicate editor-core's
/// persistent `ViewKind` identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabRole {
    #[default]
    View,
    Inspector,
    Toolbar,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub role: TabRole,
    pub widgets: Vec<WidgetId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Panel {
    pub id: PanelId,
    pub tabs: Vec<TabId>,
    pub active_tab: Option<TabId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum LayoutNode {
    Panel(PanelId),
    Split {
        id: SplitId,
        axis: SplitAxis,
        ratio: f32,
        first: Box<Self>,
        second: Box<Self>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditorUiModel {
    root: LayoutNode,
    panels: BTreeMap<PanelId, Panel>,
    tabs: BTreeMap<TabId, Tab>,
    focused_panel: Option<PanelId>,
    focused_tab: Option<TabId>,
    min_panel_size: f32,
    next_panel: u64,
    next_tab: u64,
    next_widget: u64,
    next_split: u64,
}

impl Default for EditorUiModel {
    fn default() -> Self {
        Self::new(0.08)
    }
}

impl EditorUiModel {
    pub fn new(min_panel_size: f32) -> Self {
        let panel_id = PanelId(0);
        let tab_id = TabId(0);
        let mut panels = BTreeMap::new();
        panels.insert(
            panel_id,
            Panel {
                id: panel_id,
                tabs: vec![tab_id],
                active_tab: Some(tab_id),
            },
        );
        let mut tabs = BTreeMap::new();
        tabs.insert(
            tab_id,
            Tab {
                id: tab_id,
                title: "Panel 0".into(),
                role: TabRole::View,
                widgets: Vec::new(),
            },
        );
        Self {
            root: LayoutNode::Panel(panel_id),
            panels,
            tabs,
            focused_panel: Some(panel_id),
            focused_tab: Some(tab_id),
            min_panel_size: min_panel_size.clamp(0.01, 0.49),
            next_panel: 1,
            next_tab: 1,
            next_widget: 0,
            next_split: 0,
        }
    }

    pub fn panels(&self) -> impl Iterator<Item = &Panel> {
        self.panels.values()
    }

    pub fn tabs(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.values()
    }

    pub fn panel(&self, id: PanelId) -> Option<&Panel> {
        self.panels.get(&id)
    }

    pub fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.get(&id)
    }

    pub fn focused_panel(&self) -> Option<PanelId> {
        self.focused_panel
    }

    pub fn focused_tab(&self) -> Option<TabId> {
        self.focused_tab
    }

    pub fn split_panel(&mut self, panel: PanelId) -> Result<PanelId, ModelError> {
        let bounds = self
            .snapshot()
            .panel_bounds(panel)
            .ok_or(ModelError::UnknownPanel(panel))?;
        let axis = if bounds.width >= bounds.height {
            SplitAxis::Vertical
        } else {
            SplitAxis::Horizontal
        };
        let extent = match axis {
            SplitAxis::Vertical => bounds.width,
            SplitAxis::Horizontal => bounds.height,
        };
        if extent * 0.5 < self.min_panel_size {
            return Err(ModelError::PanelTooSmall);
        }

        let new_panel = PanelId(self.next_panel);
        self.next_panel += 1;
        let new_tab = TabId(self.next_tab);
        self.next_tab += 1;
        let split = SplitId(self.next_split);
        self.next_split += 1;
        replace_panel_with_split(&mut self.root, panel, new_panel, split, axis)?;
        self.panels.insert(
            new_panel,
            Panel {
                id: new_panel,
                tabs: vec![new_tab],
                active_tab: Some(new_tab),
            },
        );
        self.tabs.insert(
            new_tab,
            Tab {
                id: new_tab,
                title: format!("Panel {}", new_panel.get()),
                role: TabRole::View,
                widgets: Vec::new(),
            },
        );
        Ok(new_panel)
    }

    pub fn remove_panel(&mut self, panel: PanelId) -> Result<(), ModelError> {
        if self.panels.len() <= 1 {
            return Err(ModelError::LastPanel);
        }
        let (root, removed) = remove_panel_node(self.root.clone(), panel);
        if !removed {
            return Err(ModelError::UnknownPanel(panel));
        }
        self.root = root.expect("removing one of multiple panels leaves a root");
        if let Some(removed_panel) = self.panels.remove(&panel) {
            for tab in removed_panel.tabs {
                self.tabs.remove(&tab);
            }
        }
        if self.focused_panel == Some(panel) {
            let replacement = self.panels.keys().next().copied();
            self.focused_panel = replacement;
            self.focused_tab = replacement.and_then(|id| self.panels[&id].active_tab);
        }
        Ok(())
    }

    /// Moves one topological split. Only the panels below that split are
    /// affected, even if another disconnected boundary has the same coordinate.
    pub fn resize_split(&mut self, split: SplitId, delta: f32) -> Result<(), ModelError> {
        let snapshot = self.snapshot();
        let edge = snapshot
            .splits
            .iter()
            .find(|edge| edge.id == split)
            .ok_or(ModelError::UnknownSplit(split))?;
        let extent = match edge.axis {
            SplitAxis::Vertical => edge.bounds.width,
            SplitAxis::Horizontal => edge.bounds.height,
        };
        let minimum_ratio = self.min_panel_size / extent;
        resize_split_node(&mut self.root, split, delta / extent, minimum_ratio)?;
        Ok(())
    }

    pub fn add_tab(
        &mut self,
        panel: PanelId,
        title: impl Into<String>,
        role: TabRole,
    ) -> Result<TabId, ModelError> {
        let panel = self
            .panels
            .get_mut(&panel)
            .ok_or(ModelError::UnknownPanel(panel))?;
        let id = TabId(self.next_tab);
        self.next_tab += 1;
        panel.tabs.push(id);
        panel.active_tab = Some(id);
        self.tabs.insert(
            id,
            Tab {
                id,
                title: title.into(),
                role,
                widgets: Vec::new(),
            },
        );
        self.focused_panel = Some(panel.id);
        self.focused_tab = Some(id);
        Ok(id)
    }

    pub fn move_tab(&mut self, tab: TabId, to_panel: PanelId) -> Result<(), ModelError> {
        if !self.tabs.contains_key(&tab) {
            return Err(ModelError::UnknownTab(tab));
        }
        if !self.panels.contains_key(&to_panel) {
            return Err(ModelError::UnknownPanel(to_panel));
        }
        let from_panel = self
            .panels
            .values()
            .find(|panel| panel.tabs.contains(&tab))
            .map(|panel| panel.id)
            .ok_or(ModelError::UnknownTab(tab))?;
        if from_panel != to_panel {
            let source = self.panels.get_mut(&from_panel).expect("source exists");
            source.tabs.retain(|candidate| *candidate != tab);
            if source.active_tab == Some(tab) {
                source.active_tab = source.tabs.first().copied();
            }
            let destination = self.panels.get_mut(&to_panel).expect("destination exists");
            destination.tabs.push(tab);
            destination.active_tab = Some(tab);
        }
        self.focused_panel = Some(to_panel);
        self.focused_tab = Some(tab);
        Ok(())
    }

    pub fn activate_tab(&mut self, tab: TabId) -> Result<(), ModelError> {
        let panel = self
            .panels
            .values_mut()
            .find(|panel| panel.tabs.contains(&tab))
            .ok_or(ModelError::UnknownTab(tab))?;
        panel.active_tab = Some(tab);
        self.focused_panel = Some(panel.id);
        self.focused_tab = Some(tab);
        Ok(())
    }

    pub fn allocate_widget(&mut self, tab: TabId) -> Result<WidgetId, ModelError> {
        let tab = self.tabs.get_mut(&tab).ok_or(ModelError::UnknownTab(tab))?;
        let id = WidgetId(self.next_widget);
        self.next_widget += 1;
        tab.widgets.push(id);
        Ok(id)
    }

    pub fn snapshot(&self) -> LayoutSnapshot {
        let mut snapshot = LayoutSnapshot::default();
        snapshot_node(&self.root, Rect::FULL, &mut snapshot);
        snapshot
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutSnapshot {
    pub panels: Vec<PanelSnapshot>,
    pub splits: Vec<SplitSnapshot>,
}

impl LayoutSnapshot {
    pub fn panel_bounds(&self, panel: PanelId) -> Option<Rect> {
        self.panels
            .iter()
            .find(|snapshot| snapshot.id == panel)
            .map(|snapshot| snapshot.bounds)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelSnapshot {
    pub id: PanelId,
    pub bounds: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitSnapshot {
    pub id: SplitId,
    pub axis: SplitAxis,
    pub ratio: f32,
    pub bounds: Rect,
    pub position: f32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("unknown panel {0:?}")]
    UnknownPanel(PanelId),
    #[error("unknown tab {0:?}")]
    UnknownTab(TabId),
    #[error("unknown split {0:?}")]
    UnknownSplit(SplitId),
    #[error("cannot remove the final panel")]
    LastPanel,
    #[error("panel is too small to split without violating the minimum size")]
    PanelTooSmall,
}

fn replace_panel_with_split(
    node: &mut LayoutNode,
    panel: PanelId,
    new_panel: PanelId,
    split: SplitId,
    axis: SplitAxis,
) -> Result<(), ModelError> {
    match node {
        LayoutNode::Panel(id) if *id == panel => {
            *node = LayoutNode::Split {
                id: split,
                axis,
                ratio: 0.5,
                first: Box::new(LayoutNode::Panel(panel)),
                second: Box::new(LayoutNode::Panel(new_panel)),
            };
            Ok(())
        }
        LayoutNode::Panel(_) => Err(ModelError::UnknownPanel(panel)),
        LayoutNode::Split { first, second, .. } => {
            if replace_panel_with_split(first, panel, new_panel, split, axis).is_ok() {
                Ok(())
            } else {
                replace_panel_with_split(second, panel, new_panel, split, axis)
            }
        }
    }
}

fn remove_panel_node(node: LayoutNode, target: PanelId) -> (Option<LayoutNode>, bool) {
    match node {
        LayoutNode::Panel(panel) => {
            if panel == target {
                (None, true)
            } else {
                (Some(LayoutNode::Panel(panel)), false)
            }
        }
        LayoutNode::Split {
            id,
            axis,
            ratio,
            first,
            second,
        } => {
            let (first, removed_first) = remove_panel_node(*first, target);
            let (second, removed_second) = remove_panel_node(*second, target);
            let removed = removed_first || removed_second;
            let result = match (first, second) {
                (Some(first), Some(second)) => Some(LayoutNode::Split {
                    id,
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (None, None) => None,
            };
            (result, removed)
        }
    }
}

fn resize_split_node(
    node: &mut LayoutNode,
    target: SplitId,
    ratio_delta: f32,
    minimum_ratio: f32,
) -> Result<(), ModelError> {
    match node {
        LayoutNode::Panel(_) => Err(ModelError::UnknownSplit(target)),
        LayoutNode::Split {
            id,
            ratio,
            first,
            second,
            ..
        } => {
            if *id == target {
                *ratio = (*ratio + ratio_delta).clamp(minimum_ratio, 1.0 - minimum_ratio);
                Ok(())
            } else if resize_split_node(first, target, ratio_delta, minimum_ratio).is_ok() {
                Ok(())
            } else {
                resize_split_node(second, target, ratio_delta, minimum_ratio)
            }
        }
    }
}

fn snapshot_node(node: &LayoutNode, bounds: Rect, snapshot: &mut LayoutSnapshot) {
    match node {
        LayoutNode::Panel(id) => snapshot.panels.push(PanelSnapshot { id: *id, bounds }),
        LayoutNode::Split {
            id,
            axis,
            ratio,
            first,
            second,
        } => {
            let (first_bounds, second_bounds, position) = match axis {
                SplitAxis::Vertical => {
                    let first_width = bounds.width * ratio;
                    (
                        Rect {
                            width: first_width,
                            ..bounds
                        },
                        Rect {
                            x: bounds.x + first_width,
                            width: bounds.width - first_width,
                            ..bounds
                        },
                        bounds.x + first_width,
                    )
                }
                SplitAxis::Horizontal => {
                    let first_height = bounds.height * ratio;
                    (
                        Rect {
                            height: first_height,
                            ..bounds
                        },
                        Rect {
                            y: bounds.y + first_height,
                            height: bounds.height - first_height,
                            ..bounds
                        },
                        bounds.y + first_height,
                    )
                }
            };
            snapshot.splits.push(SplitSnapshot {
                id: *id,
                axis: *axis,
                ratio: *ratio,
                bounds,
                position,
            });
            snapshot_node(first, first_bounds, snapshot);
            snapshot_node(second, second_bounds, snapshot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_round_trips_without_renderer_entities() {
        let mut model = EditorUiModel::default();
        let second = model.split_panel(PanelId(0)).unwrap();
        let inspector = model
            .add_tab(second, "Inspector", TabRole::Inspector)
            .unwrap();
        model.activate_tab(inspector).unwrap();

        let encoded = serde_json::to_string(&model).unwrap();
        let decoded: EditorUiModel = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, model);
        assert!(!encoded.contains("Entity"));
    }

    #[test]
    fn split_and_resize_enforce_minimum_panel_size() {
        let mut model = EditorUiModel::new(0.3);
        let second = model.split_panel(PanelId(0)).unwrap();
        let split = model.snapshot().splits[0].id;
        model.resize_split(split, 10.0).unwrap();
        let widths: Vec<f32> = model
            .snapshot()
            .panels
            .iter()
            .map(|panel| panel.bounds.width)
            .collect();
        assert!(widths.iter().all(|width| *width >= 0.3 - f32::EPSILON));
        model.split_panel(second).unwrap();
        assert_eq!(model.split_panel(second), Err(ModelError::PanelTooSmall));
    }

    #[test]
    fn removing_panel_collapses_only_its_topological_parent() {
        let mut model = EditorUiModel::default();
        let right = model.split_panel(PanelId(0)).unwrap();
        let bottom_right = model.split_panel(right).unwrap();
        let left_before = model.snapshot().panel_bounds(PanelId(0)).unwrap();
        model.remove_panel(bottom_right).unwrap();
        assert_eq!(model.snapshot().panel_bounds(PanelId(0)), Some(left_before));
        assert_eq!(model.snapshot().panels.len(), 2);
    }

    #[test]
    fn disconnected_equal_coordinates_have_distinct_split_ids() {
        let mut model = EditorUiModel::default();
        let right = model.split_panel(PanelId(0)).unwrap();
        model.split_panel(PanelId(0)).unwrap();
        model.split_panel(right).unwrap();
        let snapshot = model.snapshot();
        let horizontal: Vec<_> = snapshot
            .splits
            .iter()
            .filter(|split| split.axis == SplitAxis::Horizontal)
            .copied()
            .collect();
        assert_eq!(horizontal.len(), 2);
        assert_eq!(horizontal[0].position, horizontal[1].position);

        model.resize_split(horizontal[0].id, 0.1).unwrap();
        let resized = model.snapshot();
        let untouched = resized
            .splits
            .iter()
            .find(|split| split.id == horizontal[1].id)
            .unwrap();
        assert_eq!(untouched.ratio, horizontal[1].ratio);
    }

    #[test]
    fn tabs_move_without_changing_identity() {
        let mut model = EditorUiModel::default();
        let destination = model.split_panel(PanelId(0)).unwrap();
        let tab = model
            .add_tab(PanelId(0), "Viewport", TabRole::View)
            .unwrap();
        model.move_tab(tab, destination).unwrap();
        assert!(model.panel(destination).unwrap().tabs.contains(&tab));
        assert_eq!(model.tab(tab).unwrap().title, "Viewport");
    }
}
