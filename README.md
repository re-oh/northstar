# Northstar

A Rust-first, highly moddable aviation sandbox built on [Bevy](https://bevyengine.org/).
This repository is currently its foundation layer — package/asset
infrastructure, workspace bootstrap, and developer tooling — not yet the
game itself. See `northstar-asset-foundation-agent-brief.md` for the
original brief and `docs/` for design docs.

## Prerequisites

- [`rustup`](https://rustup.rs/). The exact toolchain
  (`rust-toolchain.toml`) is installed automatically the first time you run
  any `cargo` command in this repo.
- On Linux, `northstar-game` (the windowed executable) needs ALSA and udev
  dev headers: `sudo apt-get install libasound2-dev libudev-dev` (Debian/Ubuntu)
  or the equivalent for your distro.

## Common commands

```sh
# Build everything
cargo build --workspace

# Run the full test suite
cargo test --workspace

# Format check / apply
cargo fmt --all -- --check
cargo fmt --all

# Lint (CI runs this with -D warnings; it's already the workspace default
# via [workspace.lints], so a bare `cargo clippy` is already strict)
cargo clippy --workspace --all-targets

# Run the minimal windowed executable (opens a window, closes cleanly on
# window-close)
cargo run -p northstar-game

# Run the developer CLI
cargo run -p northstar-dev -- doctor
cargo run -p northstar-dev -- assets classify some_package.map.nspkg
```

## Workspace layout

| Crate | What it is |
| --- | --- |
| `northstar-core` | Package/asset identity, `.nspkg` filename classification, the experimental container codec. No Bevy dependency. |
| `northstar-bevy` | Bevy integration for the asset layer: `NorthstarAssetPlugin`, category-handler registry, typed `AssetRef<T>` loading. |
| `northstar-time` | The simulation clock: pause/scale/editor-preview, layered over Bevy's `Time<Fixed>`/`Time<Virtual>`. |
| `northstar-diagnostics` | Logging setup, build/version info, startup banner, panic reporting. |
| `northstar` | `NorthstarPlugin` — installs the above and establishes ordered startup phases. |
| `northstar-test-app` | `NorthstarTestApp` — a headless `App` (no window/renderer) with `NorthstarPlugin` installed, for tests. |
| `northstar-game` | The minimal windowed executable. |
| `northstar-editor-core` | The editor `View` lifecycle contract. No UI library chosen yet. |
| `northstar-ui` | Experimental tiled panel SDK with movable tabs and arbitrary tool/view bodies. |
| `northstar-dev` | Developer CLI (`doctor`, `assets ...`, `packages ...`, `validate ...`). |

Each crate's own doc comments (`cargo doc --workspace --open`) cover its
design in more depth; `docs/` covers cross-cutting decisions that don't
belong to one crate.

## Documentation

| Doc | Covers |
| --- | --- |
| `docs/architecture.md` | The `.nspkg`/package/asset system: why it's shaped the way it is. |
| `docs/assets.md` | The same system, but the practical "how do I use this" companion. |
| `docs/modding-boundary.md` | The data-only mod policy, and where it's actually enforced. |
| `docs/errors.md` | Why there's no universal `NorthstarError`, and the per-boundary convention instead. |
| `docs/simulation-time.md` | `SimClock`: rendered frames vs. fixed sim ticks vs. paused vs. editor-preview vs. time scale. |
| `docs/coordinates-and-units.md` | RFC (proposed, not implemented): handedness, altitude, floating origin, angles, velocity, mass, unit serialization. |
| `docs/editor-views.md` | The editor `View` lifecycle contract. |
| `docs/ui-panels.md` | The tiled panel/tab SDK and its application integration boundary. |

`AGENTS.md` is the short version for anyone (human or agent) about to
change code here — read it first.

## Profiles

- `dev` (default) — fast iteration; dependency crates (Bevy, glam, ...) are
  still built at `opt-level = 1` so debug builds aren't dominated by
  unoptimized math/rendering code.
- `release` — `opt-level = 3`, thin LTO, one codegen unit. Debug info is
  kept (not stripped) so release crashes are still symbolicated.
- `release-dev` — release optimization levels without LTO/single-codegen-unit,
  for local profiling iteration that doesn't need a final shippable binary:
  `cargo build --profile release-dev`.
