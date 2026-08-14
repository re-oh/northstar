# Editor UI model and Bevy adapter

Northstar's editor toolkit is not the shipping game UI framework. Its scope is
editor workflows: docked Views, tabs, inspectors, trees, asset browsers,
graphs, timelines, toolbars, workspace persistence, focus, selection, and
command routing. Game reuse is limited to the optional `debug-tools` feature.

## Ownership boundary

`northstar-editor-ui` owns the serializable workspace layout without depending
on Bevy. `EditorUiModel` uses stable `PanelId`, `TabId`, `WidgetId`, and
`SplitId` values. Splitting, removal, resize, tab movement, activation, focus,
and layout snapshots operate entirely on that model.

The topology is a split tree. A divider is addressed by `SplitId`, not by its
screen coordinate, so disconnected boundaries that happen to share an `x` or
`y` value remain independent.

`northstar-editor-ui-bevy` is the first renderer/input adapter. It reconciles
model IDs to Bevy entities and UI nodes. Tab body entities survive activation
and movement, allowing application-owned descendants to retain widget state.
Bevy entities are never serialized.

Applications mutate the model through typed adapter messages:

```rust,ignore
fn move_viewport_tab(
    viewport_tab: Res<ViewportTab>,
    destination: Res<InspectorPanel>,
    mut requests: MessageWriter<EditorUiRequest>,
) {
    requests.write(EditorUiRequest::MoveTab {
        tab: viewport_tab.0,
        to_panel: destination.0,
    });
}
```

## Editor-core seam

`northstar-editor-core::View` remains UI-neutral and owns persistent View
identity, lifecycle, internal state serialization, close blocking, and
workspace requests. The editor workspace will own open View instances and map
their stable identities onto `TabId` values. Hiding, moving, docking, or
placing a View in another tab container must not destroy the View.

That View-to-tab workspace adapter is deliberately the next seam; it is not
faked by putting View ownership into the Bevy ECS hierarchy. `ViewKind` remains
separate from the visual `TabRole` (`View`, `Inspector`, or `Toolbar`).
