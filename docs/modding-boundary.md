# The data-only mod boundary

This document states one policy, on its own, because it's load-bearing
enough to deserve being findable without reading all of
`docs/architecture.md`: **Northstar mods are data, never code.**

## The rule

A `.nspkg` package cannot ship:

- a Rust plugin or dynamic/native library;
- a WebAssembly module;
- a script (Lua, or anything else) that the engine interprets as
  instructions rather than data;
- a custom Bevy system, asset loader, or category handler;
- a reflected/constructed arbitrary Rust type named by untrusted content.

The engine owns every runtime asset type, every category decoder, every
simulation system, and every interpreter. Mods configure and compose those
capabilities *through data* — parameters, tables, curves, prefab
composition, state machines, and (eventually) event/behavior graphs built
from engine-provided nodes. All of those remain engine-executed and
bounded; none of them is an escape hatch back to arbitrary code execution.

## Where this is enforced, not just stated

- [`NspkgAssetApp::register_nspkg_asset`] is the *only* way to bind a
  `.nspkg` category to a runtime type and decoder, and it's Rust code
  called at app-setup time — never something triggered by a `.nspkg`
  file's bytes.
- An asset category is open-ended at the *classification* level
  (`AssetCategory` will happily classify a category this build has never
  heard of — see `docs/architecture.md`), but *loading* an unregistered
  category fails with an actionable error. Being nameable in a filename is
  not being executable.
- The experimental container format (`northstar_core::container`) carries
  opaque byte chunks with a `kind` label and a compression marker — never
  a function pointer, a type name to reflect against, or anything else
  that could be interpreted as "run this."
- Container parsing (`ContainerReader::parse`) is defensive against
  corrupt/adversarial bytes by construction (bounds-checked, no panics —
  see `docs/errors.md`) precisely because mod content is untrusted input by
  definition, distribution and moderation notwithstanding.

## What this deliberately does not cover

- **Steam Workshop moderation/distribution** is a separate concern —
  Northstar relies on it for discovery/download, but the data-only
  boundary above holds regardless of how a package arrived (Workshop,
  fallback install, loose dev directory).
- **Performance/anti-cheat sandboxing** of data-driven mechanisms (e.g. a
  future behavior-graph interpreter) is that mechanism's own design
  problem when it's built — this document only commits to "engine-executed
  and bounded," not to a specific enforcement technique.
- **Editor-authored vs. hand-authored content** — the boundary applies
  identically either way; nothing here assumes content comes from a
  particular tool.

## When this needs revisiting

If a future Northstar feature seems to require code mods to be useful,
that's a decision significant enough to warrant its own RFC (see
`docs/coordinates-and-units.md` for the shape such a document should take)
— not a quiet exception carved into one category's loader.

[`NspkgAssetApp::register_nspkg_asset`]: ../crates/northstar-bevy/src/registry.rs
