# Northstar launcher

`northstar-launcher` is Northstar's default executable and the future central
place for selecting and configuring installed packages, choosing profiles,
and starting an application.

The `0.1.0-dev` MVP intentionally contains only two actions:

- **Launch Northstar** starts `northstar-game`;
- **Launch Northstar Editor** starts `northstar-editor`.

When sibling binaries are available, the launcher starts them directly. Debug
builds may fall back to `cargo run -p <package>` when the workspace can be
found; release builds never depend on Cargo or a source checkout.

The launcher owns each child handle while it is open, prevents duplicate
launches, reaps completed children, and reports spawn/exit failures in the UI.
Closing the launcher deliberately detaches applications that are still
running. The launcher uses Bevy UI directly and does not depend on editor UI
internals.
