# AGENTS.md

Conventions for anyone (human or agent) working in this repository.

## Layout

- `crates/northstar-core` — dependency-light asset/package identity types,
  the `.nspkg` filename classifier, and the experimental container codec.
  No Bevy dependency, ever. This is the crate every other crate builds on.
- `crates/northstar-bevy` — Bevy integration: `NorthstarAssetPlugin`, the
  custom `northstar://` asset source, the category-handler registry, and
  `AssetRef<T>` loading. Depends on `northstar-core` + `bevy`.
- `crates/northstar-time` — the simulation clock (`SimClock`): pause, time
  scale, editor-preview, layered over Bevy's `Time<Fixed>`/`Time<Virtual>`.
  No simulation logic — see `docs/simulation-time.md`.
- `crates/northstar-diagnostics` — logging setup, build/version info,
  startup banner, panic reporting. Does not define error types — see
  `docs/errors.md`.
- `crates/northstar` — `NorthstarPlugin`: installs the crates above and
  orders the `NorthstarPhase` startup sets. No windowing, no gameplay.
- `crates/northstar-test-app` — `NorthstarTestApp`, a headless `App` (no
  window/renderer) with `NorthstarPlugin` installed, for tests. Derefs to
  `bevy::app::App`.
- `crates/northstar-game` — the minimal windowed executable. The only crate
  allowed a full-featured (non-`default-features = false`) `bevy`
  dependency, since opening a window is its entire job.
- `crates/northstar-editor-core` — the editor `View` trait skeleton. No UI
  library chosen yet — see `docs/editor-views.md`.
- `crates/northstar-dev` — the developer CLI (`doctor` / `packages ...` /
  `assets ...` / `validate ...`). `assets` absorbs the `.nspkg` tooling
  (`classify` / `inspect` / `pack-test` / `unpack-test`). Depends on
  `northstar-core` only — never add a Bevy dependency here.
- `docs/` — design rationale and the decisions this codebase has
  deliberately deferred, one document per cross-cutting concern
  (`architecture.md`, `assets.md`, `modding-boundary.md`,
  `simulation-time.md`, `coordinates-and-units.md`, `editor-views.md`,
  `errors.md`). Read the relevant one before changing identity, filename,
  container, time, or editor-view semantics.

## Ground rules

- Mods are data-only. Nothing in this repository should give package content
  a way to run code (no scripting hooks, no dynamic loaders, no reflected
  arbitrary-type construction from untrusted bytes).
- `AssetCategory` is open-ended by design — do not turn it into a closed
  enum. Unrecognized categories must still classify successfully; only
  *loading* an unregistered category is expected to fail.
- Package identity (`PackageId`) is deliberately opaque. Do not canonize a
  Steam Workshop id, filesystem path, or display name as a `PackageId` — see
  `docs/architecture.md`.
- The `.nspkg` container format is versioned and explicitly experimental
  (`FormatVersion::EXPERIMENTAL_V0`). Do not present it as stable, and keep
  encode/decode isolated behind `northstar_core::container` so it can be
  replaced.
- Keep `northstar-core` free of `unsafe` and free of panics on untrusted
  input (filenames, container bytes) — return a typed error instead.

## Before changing things

- Run `cargo fmt`, `cargo test --workspace`, and
  `cargo clippy --workspace --all-targets -- -D warnings` before considering
  a change done.
- Preserve unrelated in-progress work in a dirty worktree; do not clean up
  files you didn't touch.
