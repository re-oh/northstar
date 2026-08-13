# Northstar launcher

`northstar-launcher` is Northstar's default executable and the future central
place for selecting and configuring installed packages, choosing profiles,
and starting an application.

The `0.1.0-dev` MVP intentionally contains only two actions:

- **Launch Northstar** starts `northstar-game`;
- **Launch Northstar Editor** starts `northstar-editor`.

When sibling binaries are available, the launcher starts them directly. In a
Cargo development checkout it falls back to `cargo run -p <package>`, allowing
the launcher to remain useful without an installation or packaging workflow.
