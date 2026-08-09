# AGENTS.md

Conventions for anyone (human or agent) working in this repository.

## Layout

- `crates/northstar-core` — dependency-light asset/package identity types,
  the `.nspkg` filename classifier, and the experimental container codec.
  No Bevy dependency, ever. This is the crate every other crate builds on.
- `crates/northstar-bevy` — Bevy integration: `NorthstarAssetPlugin`, the
  custom `northstar://` asset source, the category-handler registry, and
  `AssetRef<T>` loading. Depends on `northstar-core` + `bevy`.
- `crates/northstar-cli` — a small inspection/round-trip tool
  (`classify` / `inspect` / `pack-test` / `unpack-test`). Depends on
  `northstar-core` only — never add a Bevy dependency here.
- `docs/architecture.md` — design rationale and the decisions this codebase
  has deliberately deferred. Read it before changing identity, filename, or
  container semantics.

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
