# Editor `View` contract

`northstar-editor-core` defines the lifecycle every editor panel/tab
implements (`View`, `docs/editor-views.md` is that crate's companion
document — read `crates/northstar-editor-core/src/view.rs`'s doc comments
alongside this). **No UI library is chosen or built yet.** This document
covers what's decided (the contract) and what deliberately isn't (which
crate renders it).

## What a `View` is

A `View` is one open editor panel or tab: a map editor, an asset browser, a
property inspector, a log console. It:

- has a stable identity ([`ViewId`]) for the lifetime of the open instance;
- has a kind ([`ViewKind`], an open string — same "don't close it off"
  reasoning as `northstar_core::AssetCategory`);
- has a display title that can change over time (e.g. an unsaved-changes
  indicator);
- owns its own persistent internal state — the workspace does not track
  what's inside a view, only when to call its lifecycle methods and where
  to place it.

## Lifecycle

- **Activation** (`on_activate`/`on_deactivate`) — called when a view
  becomes or stops being the focused view. Default: no-op. A view that
  needs to do something when it (re)gains focus (refresh a list, resume
  polling) hooks in here; a view that doesn't, doesn't need to override
  anything.
- **Closing** (`on_close`) — returns [`CloseResponse::Allow`] or
  [`CloseResponse::Block`]. `Block` is how a view says "I have unsaved
  changes, don't close me silently" — the workspace deciding what a block
  actually *looks like* (a modal? an inline prompt?) is a UI decision, not
  this crate's.
- **Serialization** (`serialize_state`/`deserialize_state`) — opaque bytes
  in, opaque bytes out. This crate does not impose an encoding; a view
  implementation picks whatever fits it (RON is already a workspace
  dependency via other crates, but nothing requires using it specifically).
  `deserialize_state` must treat its input as untrusted — a saved workspace
  layout file can be stale, hand-edited, or from an older Northstar version
  — and return `ViewStateError::Restore` rather than panic, matching
  `docs/errors.md`'s rule for anything parsing external bytes.

## Tabs

Whether a given `View` instance is presented as a standalone panel or as
one of several tabs in a container is a **workspace layout decision**, not
something this trait distinguishes. What the trait *does* capture is
registration-time information relevant to that decision:
[`ViewDescriptor::singleton`] says whether the workspace should ever allow
more than one open instance of a kind at once (a single asset browser vs.
several simultaneously-open map editors, say). The registry that would map
[`ViewKind`] → [`ViewDescriptor`] (analogous to `northstar_bevy`'s category
registry) doesn't exist yet — `ViewDescriptor` is defined so its shape is
already settled once that registry gets built.

## Talking back to the workspace

A view's lifecycle methods receive a [`ViewContext`], wrapping a
[`WorkspaceHandle`] — a small, object-safe trait a view uses to request
opening/closing views, mark itself dirty, or request focus. Nothing
implements `WorkspaceHandle` yet; it exists so a view's *interface* to the
workspace is fixed before the workspace itself is built, the same way
`NorthstarLoadContext` fixed the asset-loading interface before every
decoder existed.

## Why no UI library yet

Bevy's own UI story (`bevy_ui`, `bevy_feathers`, egui-via-`bevy_egui`, or
something else entirely) is a bigger decision than this document's scope,
and picking one prematurely would make every view implementation depend on
that choice before there's a second view to validate it against. What *is*
locked in now is the shape a view needs to have regardless of which UI
library eventually renders it — identity, kind, title, lifecycle,
serialization, and a narrow interface back to the workspace.

## What's not decided here

- Which UI library/immediate-mode-vs-retained-mode approach the workspace
  is built on.
- The actual `WorkspaceHandle` implementation, docking/layout algorithm, or
  how tabs are visually grouped.
- The view registry (`ViewKind` → `ViewDescriptor`) — `ViewDescriptor`'s
  shape is fixed, the registry holding them is not built.
- State serialization's concrete encoding — left to each view.

[`ViewId`]: ../crates/northstar-editor-core/src/view_id.rs
[`ViewKind`]: ../crates/northstar-editor-core/src/view_kind.rs
[`CloseResponse::Allow`]: ../crates/northstar-editor-core/src/view.rs
[`CloseResponse::Block`]: ../crates/northstar-editor-core/src/view.rs
[`ViewDescriptor::singleton`]: ../crates/northstar-editor-core/src/view.rs
[`ViewContext`]: ../crates/northstar-editor-core/src/workspace.rs
[`WorkspaceHandle`]: ../crates/northstar-editor-core/src/workspace.rs
