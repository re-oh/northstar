# Northstar asset architecture

This document covers the design of Northstar's first technical foundation:
the `.nspkg` package/asset model, package-local identity, the experimental
container format, and the Bevy integration built on top of them. It exists
so the *reasons* behind fixed decisions and deferred ones survive past the
code that implements them.

Scope note: this is a small, tested architectural proof, not a production
asset pipeline. Real content schemas (maps, missions, prefabs, aircraft,
weapons, sensors...), an editor, a final package-identity encoding, and
Steamworks integration are all explicitly out of scope — see "Deferred
decisions" below and each crate's own doc comments.

## The two filename forms

All packaged Northstar content uses the `.nspkg` extension, and a filename
is one of exactly two shapes:

```text
<package_puid>.nspkg
<asset_puid>.<asset_category>.nspkg
```

| Filename | Classification |
| --- | --- |
| `basegame.nspkg` | Complete content package |
| `pebble_sea_islands.map.nspkg` | Map asset |
| `oil_rig_protection.mission.nspkg` | Mission asset |
| `garrisoned_oil_rig.prefab.nspkg` | Prefab asset |

The compound filename is intentional: tools must be able to tell a complete
package apart from an individually packaged asset — and, for an asset,
determine its category — **without opening or reading the file**. This
matters for anything that indexes, copies, or reports on a package
directory (including a future editor asset browser) without wanting to pay
for a parse of every file just to draw a list.

## Filename-only classification

[`northstar_core::filename::ClassifiedFilename::classify`] implements the
grammar: after removing the `.nspkg` suffix, a final `.` separates an asset
PUID from its category. With no remaining `.`, the file is a complete
package.

This is why [`PackageId`], [`AssetPuid`], and [`AssetCategory`] all reject a
literal `.` in their own validation (`northstar_core::ident`) — a segment
containing a dot would make the "parse from the right" rule ambiguous. A
package named `my.package` would misparse as an asset with PUID `my` and
category `package`. Rejecting dots at the identity-type level, rather than
only in the classifier, means the invariant holds everywhere these types are
constructed, not just at the one call site that happens to check it today.

Malformed names (empty PUID, empty category, missing extension) return a
structured [`ClassifyError`] — the classifier never guesses or silently
normalizes an identity. Classification is also independent of content
validation: a filename can classify successfully even when the file is
missing or its bytes are corrupt. Whether the *bytes* are well-formed is a
separate question, answered by [`northstar_core::container`], not by the
filename.

An **unrecognized-but-well-formed category still classifies as an asset**.
`AssetCategory` is an open-ended, validated string — never a closed enum —
specifically so that indexing, copying, and reporting tools keep working
against a category a given build of Northstar doesn't know how to load yet.
Only *loading* an unregistered category is expected to fail (see
"Category evolution" below).

## Package-local asset identity

An [`AssetPuid`] is unique only **within its owning package**. Northstar
does not require globally coordinated string identifiers for every asset
created by every mod author, so two different packages may legally contain
assets with the same PUID — `basegame`'s `oil_rig_protection` and
`some_mod`'s `oil_rig_protection` are different assets.

Canonical asset identity is therefore the package-qualified pair
`(owning_package_identity, asset_puid)`, modeled as
[`northstar_core::key::AssetKey`] — a real product type with `package()` and
`puid()` accessors, not a loose tuple or a string convention scattered
through the codebase. `AssetCategory` is deliberately **excluded** from this
identity: category is classification/type information (which decoder to
use), not part of *which* asset something is. Two differently-categorized
files can never collide as the same `AssetKey` in the first place, since the
category isn't part of the key — but the deeper reason to keep it out is
that an asset's category is allowed to matter for *how* it's loaded without
mattering for *whether it's the same asset* as some other reference to it.

Code should hold an `AssetKey` (or, on the Bevy side, an
[`northstar_bevy::AssetRef<T>`]) rather than a bare `AssetPuid` string used
as a cross-package key.

### `PackageId` is intentionally opaque

The final encoding and generation policy for durable package identities has
**not been chosen**. [`PackageId`] exists precisely so that decision can be
deferred without every call site depending on whatever stand-in is
convenient today. In particular: **do not** canonize a Steam Workshop item
ID, a filesystem path, or a display name as a `PackageId`. For this
experimental slice, `PackageId` is validated as a filename-safe segment
(non-empty, ASCII alphanumeric plus `_`/`-`, no `.`), which is enough to
round-trip through the `.nspkg` grammar and a mounted-directory catalog, but
is *not* claimed to be the permanent identity format.

## Steam Workshop is distribution, not identity

Northstar relies on Steam Workshop for mod discovery, download, updating,
and distribution. None of that is built here — no competing workshop, no
repository service, no downloader, no account system, no Steamworks API
integration. What this slice does guarantee is the *shape* of the
separation: a `PackageId` never depends on a Steam Workshop item ID, so a
package's identity and its asset references stay meaningful if it's
installed from a fallback source, and Steam-as-distribution can be modeled
later as an association *from* a `PackageId`, not folded into it.

## Category evolution and the data-only mod boundary

Northstar does not support code mods at this stage. A `.nspkg` package
cannot ship a Rust plugin, a native library, a WebAssembly module, a
script, a custom Bevy system, or a custom asset loader — the engine owns
every runtime asset type, category handler, and interpreter; mods configure
and compose those capabilities through data.

This boundary is enforced structurally, not just by convention:

- [`northstar_bevy::NspkgAssetApp::register_nspkg_asset`] is the *only* way
  to bind a category to a runtime asset type and decoder, and it is engine
  (Rust) code, called at app-setup time — never something a `.nspkg` file's
  bytes can trigger.
- The category → type registry
  (`northstar_bevy::registry`, deliberately not exposed as a public type) is
  populated once per type; a data file can name a category in its filename,
  but naming a category is not the same as being able to register one.
  Registering the same `(type, category)` pair twice is a no-op (useful
  across multiple `App`s in one process, e.g. tests); registering a
  *conflicting* binding panics loudly and immediately, because that is a
  static configuration bug, not a runtime condition to route around.
- An unrecognized category can still be classified, catalogued, indexed,
  and reported by tooling — the filename grammar doesn't care whether the
  category is "known". But **loading** an asset whose category has no
  registered decoder fails with an actionable error
  ([`NspkgLoadError`] / a Bevy `LoadState::Failed`), not a panic and not a
  silent no-op. See `crates/northstar-bevy/tests/nspkg_loading.rs` for this
  exact scenario exercised end-to-end.

A new runtime category or simulation capability requires a Northstar engine
update. The data-driven extension mechanisms this is expected to grow
toward — component/module definitions, parameter tables and curves, prefab
composition, state machines, behavior graphs built from engine-provided
nodes, references between package-local assets — all stay engine-executed
and bounded. None of them are implemented in this slice; the point of this
section is the boundary they'll eventually live inside, not their design.

## The binary format is a container, and it's experimental

A `.nspkg` binary doesn't imply every resource inside it uses a bespoke raw
byte layout. The format
([`northstar_core::container`]) is a small header (magic, format version,
self-reported identity, a metadata payload) followed by an index of opaque
chunks, followed by the chunk bytes. A chunk can eventually hold structured
metadata, an existing file format (model/texture/sound/...), a `tar.gz` of
several files, compressed or uncompressed data, derived runtime data, or
editor-only source information — the container doesn't care, and
compression is a property of an individual chunk
(`ChunkDescriptor::compression`), not an assumption baked into the whole
file. Version zero implements exactly one compression representation
(`ChunkCompression::None`); the tag exists in the wire format specifically
so a later version can add real compression without changing the chunk
model's shape.

**This layout is explicitly experimental and not a compatibility promise.**
`FormatVersion::EXPERIMENTAL_V0` is a real value precisely so nothing has to
pretend otherwise. There is no in-place editing, no streaming, no
encryption, no signatures, no deduplication, and no content-cooking
strategy — all deferred. Every reader/writer path is confined behind
`ContainerReader`/`ContainerWriter` so the on-disk layout can be replaced
without the rest of Northstar (or any decoder written against this API)
noticing.

The reader validates bounds, string UTF-8, and a per-chunk FNV-1a checksum
at parse time and returns a typed [`ContainerError`] on truncated, corrupt,
or adversarial input — it never panics. This is ordinary defensive parsing
for corruption and bad input, not DRM or a moderation mechanism; Steam
Workshop already owns distribution and moderation (see above).

The container also carries the classified filename's identity (package vs.
asset, and for an asset, its PUID/category) in its own header. This lets a
loader catch an "obvious mismatch" — a file whose bytes claim to be a
`mission` while its filename says `map` — before decoding proceeds; see
`NspkgLoadError::IdentityMismatch`.

## The boundary between core asset code and Bevy integration

`northstar-core` (identity types, the filename classifier, the container
codec) has **no Bevy dependency** and never will. `northstar-bevy` depends
on it, not the other way around. This split exists so the addressing and
container model can be tested, used from tooling (`northstar-cli`), and
reasoned about independent of any particular Bevy version or even of Bevy
existing at all in a given binary.

On the Bevy side, Northstar is an addressing, package-mounting, and
container-decoding layer *beneath* Bevy's normal typed asset system — it is
not a replacement for Bevy's asset storage, and it never hands gameplay
code an untyped blob to downcast:

- [`NorthstarAssetPlugin`] registers a named `northstar://` asset source
  (see [`bevy::asset::io::AssetSource`]), backed by a
  [`PackageCatalog`]. This slice implements exactly one catalog backend — a
  loose development directory with one subdirectory per mounted
  `PackageId` — reusing Bevy's own platform-default file reader rather than
  a hand-rolled `AssetReader` impl, since the file-serving behavior needed
  here is exactly what Bevy's file backend already does correctly. Steam
  installations, a fallback installation location, and archive members are
  future catalog backends; nothing about the plugin, the `northstar://`
  path shape, or `AssetRef<T>` assumes a single flat directory is the only
  backend that will ever exist — gameplay code never sees the physical
  backing storage at all.
- Must be added **before** `AssetPlugin` (typically part of
  `DefaultPlugins`) — Bevy builds registered asset sources when
  `AssetPlugin` itself builds, so a source registered afterward is invisible
  to the running `AssetServer`. This is a Bevy 0.19 requirement, not a
  Northstar one; see `bevy_asset::AssetApp::register_asset_source`'s own
  doc comment.
- [`NspkgAssetApp::register_nspkg_asset::<A, D>(category, decoder)`] binds
  one category to one runtime asset type `A` and one [`NspkgDecoder`] `D`.
  Multiple `NspkgLoader<D>` instances can all declare the `nspkg` extension
  because Bevy resolves a *typed* load (`asset_server.load::<A>(path)`) by
  the requested asset type first — `AssetLoaders::find` in `bevy_asset`
  looks up loaders by `TypeId` before falling back to extension-only
  matching — so requesting `Handle<MissionAsset>` and `Handle<MapAsset>`
  against files that share the `.nspkg` extension resolves unambiguously as
  long as each registered type has exactly one loader. This is why decoders
  and gameplay code are required to use typed loads
  ([`NorthstarLoadContext::load`]/[`load_local`], [`NorthstarAssets::load`])
  and why direct untyped `.nspkg` loading was deliberately left out of the
  supported API — an untyped load can only resolve by extension, which
  *is* ambiguous across categories.
- [`AssetRef<T>`] is a strong, package-qualified, typed reference
  (`PackageId` + `AssetPuid`, generic over the runtime asset type). Building
  one never touches the filesystem or the category registry; resolution —
  looking up which category `T` was registered under, validating that a
  category exists for `T` at all, constructing the internal
  `northstar://<package>/<puid>.<category>.nspkg` path, and performing the
  actual typed load — happens in [`NorthstarAssets::load`]. Gameplay code
  never constructs that path string by hand.
- Inside a decoder, [`NorthstarLoadContext`] wraps Bevy's own `LoadContext`
  and exposes `load_local::<T>(puid)` (same package as the asset currently
  being decoded) and `load::<T>(&AssetKey)` (any package, always explicit).
  Both return an ordinary `Handle<T>` produced through Bevy's real load
  path, so Bevy's dependency tracking, `Assets<T>`, and asset events all
  work exactly as they would for any other Bevy asset — there is no
  parallel handle or lifetime system layered on top.

### Why the category registry is a process-global table, not an ECS resource

This is the one deliberately unusual implementation choice in
`northstar-bevy`, so it's worth calling out on its own. Bevy's
`LoadContext` (what a decoder receives while running inside an
`AssetLoader::load` future) does not expose the `World` or its resources —
there is no path from inside a decoder back to "which category string did
`MapAsset` get registered under" through the ECS. Since both
`NorthstarLoadContext` (inside a decoder) and `NorthstarAssets` (gameplay
code, a `SystemParam`) need that same lookup, `northstar_bevy::registry`
keeps it in a small `OnceLock<RwLock<..>>` behind `category_for::<A>()`
instead. Consequences worth being explicit about:

- Registration is idempotent for a repeated `(type, category)` pair — this
  matters because multiple `App`s can exist in one test process — but
  panics immediately on a genuine conflict (same type bound twice to
  different categories, or the same category claimed by two types).
- This is a provisional choice, isolated behind the `category_for`/register
  functions specifically so it can be replaced (e.g. if a future Bevy
  version threads resources through `LoadContext`, or if multiple
  independent `AssetServer`s per process ever becomes a real requirement)
  without touching `NorthstarLoadContext` or `NorthstarAssets` call sites.

## Deferred decisions

Deliberately not decided or built in this slice — see each crate's
non-goals in the agent brief this codebase was built from:

- the permanent `PackageId` encoding and issuance authority;
- the permanent binary layout, compression algorithms, archive policy, and
  content-cooking/streaming strategy for `.nspkg` (version zero is
  explicitly experimental);
- a final package manifest format;
- Steamworks API integration and an alternative Workshop service;
- dependency downloading or version solving;
- signatures, DRM, or encryption;
- asset inheritance, patching, overriding, or load-order semantics;
- multiplayer synchronization;
- real map/mission/prefab/aircraft/vehicle/weapon/sensor/terrain schemas,
  editor UI, thumbnails, or asset browsers;
- Steam / fallback-installation / archive-member catalog backends beyond
  the loose development directory implemented here;
- performance work unsupported by measurements.

Where an implementation detail forced a provisional choice not on this
list, it's called out at the point it was made (e.g. the FNV-1a chunk
checksum, or the process-global category registry above) rather than
collected here.
