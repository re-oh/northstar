# Panel UI SDK

`northstar-ui` provides layout and shell mechanics, not a catalog of editor
tools. Each panel has a tab strip and a content slot. Each `PanelTab` owns a
plain Bevy UI body entity; applications attach viewport, inspector, toolbar,
button, slider, or other widget entities beneath that body.

Add `NorthstarUiPlugin::default()` for the development controls, or
`NorthstarUiPlugin::sdk()` when the application supplies its own bindings.
The application owns the UI camera.

Layout operations use typed Bevy messages:

```rust,ignore
fn move_viewport_tab(
    viewport_tab: Res<ViewportTab>,
    destination: Res<InspectorPanel>,
    mut requests: MessageWriter<PanelRequest>,
) {
    requests.write(PanelRequest::move_tab(viewport_tab.0, destination.0));
}
```

Custom systems that write `PanelRequest` during `Update` should run in or
before `PanelUiSet::Input`; requests are applied in `PanelUiSet::Mutate`.

`PanelTabKind` is intentionally small for now:

- `View` for viewports and general tool surfaces;
- `Inspector` for property/editor panels;
- `Toolbar` for horizontal collections of arbitrary controls.

The kind supplies initial body layout only. It does not restrict what widgets
the application may place inside the body.

## MVP primitives

`NorthstarUiPrimitivesPlugin` installs the shared `UiTheme` and interaction
colors for `UiButton`. It does not create a workspace. The current primitive
bundles are deliberately small:

- `UiSurfaceBundle` for a full-window application surface;
- `UiStackBundle` for simple row/column composition;
- `UiButtonBundle` with primary and secondary tones.

These are WIP `0.1.0-dev` building blocks for the launcher and editor. They
will evolve as real controls such as sliders, fields, menus, and toolbars are
developed.
