# Northstar Asset Foundation — Agent Implementation Brief

## Role

You are establishing Northstar's first technical foundation: a unified asset scheme and the Bevy tooling that supports it. Northstar is a Rust-first, highly moddable aviation sandbox. Aircraft, maps, missions, prefabs, vehicles, weapons, sensors, UI resources, and future mod-defined content must ultimately fit into the same asset system.

Keep this first slice deliberately small. Implement the naming, identity, container, and Bevy-integration seams needed to prove the design. Do not build the game's actual content types or a complete production asset pipeline.

Before editing, inspect the repository, its `AGENTS.md` files, workspace layout, pinned Rust toolchain, Bevy version, existing conventions, and current tests. Preserve compatible existing work. If the repository is empty, create only the minimal Rust workspace needed for this task.

## Product principles

- Northstar must remain modular and easy to modify, extend, and replace in parts.
- Asset infrastructure must not hard-code the current game systems, editor layout, storage assumptions, or a closed set of asset categories.
- Modding is a core capability, not an afterthought.
- Prefer small stable interfaces, typed boundaries, explicit schemas, and replaceable implementations.
- Do not create premature abstractions unrelated to the first working asset slice.

## Fixed design decisions

Treat the following as requirements, not suggestions.

### One Northstar container extension

All packaged Northstar content uses the `.nspkg` extension. `nspkg` means **Northstar package**.

The filename itself distinguishes a complete content package from an individually packaged asset:

```text
<package_puid>.nspkg
<asset_puid>.<asset_category>.nspkg
```

Examples:

```text
basegame.nspkg
pebble_sea_islands.map.nspkg
oil_rig_protection.mission.nspkg
garrisoned_oil_rig.prefab.nspkg
```

Their classifications are:

| Filename | Classification |
| --- | --- |
| `basegame.nspkg` | Complete content package |
| `pebble_sea_islands.map.nspkg` | Map asset |
| `oil_rig_protection.mission.nspkg` | Mission asset |
| `garrisoned_oil_rig.prefab.nspkg` | Prefab asset |

The compound filename is intentional. Tools must be able to determine whether a path represents a complete package or an individual asset—and determine an individual asset's category—without opening or reading the file.

Parse from the right. After removing `.nspkg`, a final dot separates an asset's PUID from its category. With no category segment, the file represents a complete package. Document the minimum filename grammar necessary to keep that rule unambiguous, but do not invent additional naming restrictions without a demonstrated need.

An unrecognized category must still classify as an individual asset. Classification must not depend on a closed built-in category list: tools must be able to index, copy, inspect, and report a future or unsupported category without misclassifying it as a complete package. Runtime decoding is separate and requires an engine-owned handler for that category.

### Package-local asset identity

An asset PUID is unique within its owning content package. Northstar must not require globally coordinated string identifiers for every asset created by every mod author.

Represent canonical asset identity as a package-qualified value conceptually equivalent to:

```text
(owning_package_identity, asset_puid)
```

The asset category is classification/type information, not the asset's globally coordinated identity. Do not use a bare `asset_puid` as a cross-package key. Two packages may legally contain assets with the same asset PUID and those assets must remain distinct.

Keep package identities and asset PUIDs as separate strong types. Do not spread unstructured path strings or tuple conventions through the codebase.

The final encoding and generation policy for durable package identities has not yet been chosen. Isolate that decision behind a strong type and parsing/formatting boundary; do not silently canonize a Steam ID, filesystem path, display name, or arbitrary library-specific identifier as the permanent package identity.

### Steam Workshop is distribution, not identity

Northstar will rely heavily on Steam Workshop for mod discovery, download, updating, and distribution. Do not build a competing workshop, repository service, network downloader, or account system in this task.

However, Steam Workshop item IDs must not be the durable identities of Northstar packages. Model Steam as an optional distribution binding associated with a Northstar package identity. The asset and package model must survive installation from a fallback source if Steam Workshop eventually becomes unavailable.

Do not implement Steam integration in this slice. Only ensure the core model does not preclude it or depend upon it.

### Mods are data-only

Northstar does not support code mods at this stage. Mod packages must not provide Rust plugins, native libraries, WebAssembly modules, arbitrary scripts, custom Bevy systems, custom asset loaders, or other executable code.

The engine owns all runtime asset types, category handlers, simulation systems, and interpreters. Mods configure and compose those capabilities through data. When Northstar implements a gameplay capability, it should expose the data structures needed to make useful content variations rather than hard-code individual aircraft, weapons, sensors, vehicles, missions, or prefabs.

Expected data-driven extension mechanisms may eventually include:

- component and module definitions;
- parameter sets, tables, and curves;
- prefab composition;
- state machines;
- event/action or behavior graphs made from engine-provided nodes;
- references between package-local assets.

These mechanisms remain engine-executed and bounded. They are not an escape hatch for arbitrary code execution.

A data-only mod may create unlimited assets using categories and schemas understood by the installed Northstar version. A new runtime category or new simulation capability requires a Northstar engine update. Unknown categories must remain discoverable and diagnosable by tooling, but the runtime must reject attempts to instantiate them until the engine supports them.

### The binary format is a container

A custom `.nspkg` binary format does not imply that every contained resource must use a bespoke raw-byte serialization.

The container must be able to carry opaque named or typed byte chunks. A chunk may eventually contain, for example:

- structured metadata;
- a model, texture, sound, or other existing file format;
- a `tar.gz` archive containing several files;
- compressed or uncompressed data;
- derived runtime data;
- editor-only source information.

Design the format so compression is a property of a chunk or payload representation rather than an assumption that the entire file always uses one compression scheme. Do not implement every compression option now.

The permanent binary layout, compression algorithms, archive policy, content cooking strategy, and streaming strategy are unresolved. Any initial layout must be clearly versioned as an experimental format and confined behind reader/writer interfaces so it can be replaced without infecting the rest of Northstar.

## First implementation slice

Deliver a minimal end-to-end proof consisting of the following pieces. Adapt names to existing repository conventions rather than reorganizing unrelated code.

### 1. Core asset model

Create a dependency-light Rust library containing strong types for at least:

- durable package identity;
- package-local asset PUID;
- asset category;
- package-qualified asset key/reference;
- classified `.nspkg` filename;
- format/schema version;
- chunk descriptor and chunk identifier.

Types should make invalid cross-package lookups difficult. Avoid unnecessary Bevy dependencies in this core crate.

### 2. Filename classifier

Implement a pure path/filename classifier that performs no file I/O. It must distinguish complete packages from categorized asset packages using only the filename grammar.

It must correctly classify all four examples above and preserve unknown category strings. Return structured errors for malformed names; do not silently guess or normalize identities.

Keep classification separate from content validation. A filename can classify successfully even if the file is missing or its bytes are corrupt.

### 3. Experimental container codec

Implement the smallest useful versioned container reader and writer that can round-trip:

- the format magic and experimental format version;
- enough identity information to detect obvious mismatches when a file is actually loaded;
- a small metadata payload;
- an ordered/indexed collection of opaque chunks;
- each chunk's representation or compression marker, even if version zero supports only an uncompressed representation;
- sizes, offsets, and basic integrity information required for safe parsing.

The reader must validate bounds and reject malformed/truncated data without panicking. This is ordinary defensive parsing for corruption and bad input, not an attempt to create DRM or replace Steam's moderation and distribution model.

Do not optimize the layout prematurely. Do not implement in-place editing, streaming, encryption, signatures, deduplication, or production-grade cooking.

### 4. Bevy integration seam

Create a small Bevy-facing crate or module that:

- exposes a `NorthstarAssetPlugin` or equivalently clear integration point;
- registers a custom Bevy asset source backed by Northstar's mounted-package catalog;
- classifies `.nspkg` paths by filename before content decoding;
- routes categorized assets through an engine-owned category-handler registry;
- demonstrates loading one intentionally trivial test asset category into a typed Bevy asset;
- reports unsupported categories and malformed containers as useful errors;
- preserves ordinary Bevy `Handle<T>`, `Assets<T>`, asset events, and dependency tracking after loading.

Use the APIs appropriate to the Bevy version already pinned by the repository. Consult the matching official Bevy documentation rather than assuming APIs from another release.

The test category exists only to prove dispatch and loading. Do not begin implementing real maps, missions, prefabs, terrain, aircraft, or editor UI.

#### Required Bevy-facing model

Northstar is an addressing, package-mounting, and container-decoding layer beneath Bevy's normal typed asset system. It must not replace Bevy's runtime asset storage with a generic blob store.

A complete content package such as `basegame.nspkg` is mounted by Northstar and represented in a package catalog. It is not normally loaded as a gameplay `Asset`. The custom Bevy asset source exposes assets inside mounted packages through logical paths conceptually equivalent to:

```text
northstar://<package-identity>/pebble_sea_islands.map.nspkg
northstar://<package-identity>/oil_rig_protection.mission.nspkg
northstar://<package-identity>/garrisoned_oil_rig.prefab.nspkg
```

Those are internal logical locators, not globally coordinated human-authored asset names. The package catalog resolves them to a Steam installation, fallback installation, loose development directory, archive member, or other backing storage without exposing that physical location to gameplay systems.

The asset source must be registered at the point required by the pinned Bevy version, including before Bevy's `AssetPlugin` when that version requires it. Keep source/mount setup distinct from per-type loader registration.

Each engine-supported asset category maps to an ordinary typed Bevy asset and an engine-owned decoder. The intended registration API should resemble:

```rust,ignore
app.register_nspkg_asset::<MapAsset, MapDecoder>("map")
   .register_nspkg_asset::<MissionAsset, MissionDecoder>("mission")
   .register_nspkg_asset::<PrefabAsset, PrefabDecoder>("prefab");
```

The exact API may be adjusted to fit the pinned Bevy release, but it must preserve these semantics:

- one category is associated with one expected runtime asset type and decoder;
- registration is performed by Northstar engine code, never by a content package;
- loaders produce typed Bevy assets rather than a `NorthstarBlob` that callers must downcast;
- the system remains open to additional categories in future Northstar releases without defining code execution as a mod feature;
- duplicate category registrations and conflicting type/category associations fail explicitly.

Because Bevy asset loaders have a fixed associated output asset type, do not design one dynamic loader that attempts to return unrelated runtime types. Use typed loads and the pinned Bevy version's loader-selection facilities. Direct untyped loading of `.nspkg` assets should not be part of the supported gameplay API.

Game code should load a strong, package-qualified, typed asset reference through a thin Northstar wrapper and receive a normal Bevy handle:

```rust,ignore
let reference = AssetRef::<MissionAsset>::new(
    basegame_package_id,
    "oil_rig_protection",
);

let handle: Handle<MissionAsset> = northstar_assets.load(reference);
```

The wrapper resolves the owning package and package-local PUID, validates that the filename category matches the requested runtime type, constructs the internal logical path, performs a typed Bevy load, and returns `Handle<T>`. Gameplay code must not construct logical paths manually or use bare PUID strings as cross-package references.

After loading, systems consume assets normally through resources such as `Assets<MissionAsset>` and normal Bevy asset events. Do not introduce a parallel lifetime, caching, or handle system.

#### Dependencies

Asset decoders need a Northstar load context layered over Bevy's load context. It should resolve package-qualified and same-package asset references into typed `Handle<T>` dependencies while preserving Bevy's dependency tracking.

The intended decoder experience should resemble:

```rust,ignore
let map = context.load::<MapAsset>(map_reference)?;
let prefab = context.load_local::<PrefabAsset>("garrisoned_oil_rig")?;
```

`load_local` means the current owning package, not a global search path. Cross-package loading requires an explicitly package-qualified reference. The resulting runtime asset stores ordinary typed Bevy handles.

Do not implement real dependency schemas in this slice. Prove the behavior with two trivial test asset types and one dependency between them.

#### Data-only boundary

The handler registry is extensible by Northstar engine code, not by `.nspkg` content. An unknown category can be classified and catalogued without opening the file, but attempting to load it must produce a clear unsupported-category error.

Do not load dynamic libraries, invoke scripts, deserialize executable function pointers, construct arbitrary reflected Rust types named by untrusted content, or allow packages to register Bevy plugins and systems. The proof should demonstrate data-driven asset loading only.

### 5. Minimal developer tooling

Add a small command-line tool or example capable of the minimum useful inspection workflow:

```text
classify <path>
inspect <path>
pack-test <output-path>
unpack-test <input-path> <output-directory>
```

Exact command names may follow existing conventions. `classify` must use only the filename and must not read the file. `inspect` may display the experimental container header and chunk index. The pack/unpack operations only need to prove lossless opaque-chunk round-tripping; they are not the final mod-authoring interface.

### 6. Documentation

Add a concise architecture document covering:

- the two filename forms;
- filename-only classification;
- package-local asset PUIDs;
- package-qualified references;
- category evolution and the engine-owned data-only mod boundary;
- the separation of Northstar identity from Steam Workshop distribution;
- the experimental status of the binary layout;
- the boundary between core asset code and Bevy integration;
- unresolved decisions that were deliberately deferred.

Use the examples from this brief in both documentation and tests.

## Required tests

At minimum, cover these behaviors:

1. `basegame.nspkg` classifies as a complete content package without file I/O.
2. `pebble_sea_islands.map.nspkg` yields PUID `pebble_sea_islands` and category `map`.
3. `oil_rig_protection.mission.nspkg` yields PUID `oil_rig_protection` and category `mission`.
4. `garrisoned_oil_rig.prefab.nspkg` yields PUID `garrisoned_oil_rig` and category `prefab`.
5. An unknown but syntactically valid category still classifies as an asset.
6. Malformed compound filenames produce explicit errors.
7. Identical asset PUIDs owned by different packages do not compare as the same canonical asset.
8. A package-local registry rejects duplicate canonical asset identities within one package.
9. The experimental writer and reader round-trip multiple arbitrary chunks exactly.
10. Truncated headers, invalid offsets, impossible sizes, and corrupted integrity data return errors without panics.
11. The Bevy integration routes the trivial test category through its registered handler.
12. An unregistered category produces an actionable loading error rather than being misclassified as a complete package.

## Non-goals

Do not implement or decide the following in this task:

- real map, mission, prefab, aircraft, vehicle, weapon, sensor, or terrain schemas;
- editor windows, inspectors, thumbnails, or asset browsers;
- a final package manifest format;
- a final package-identity encoding or issuance authority;
- Steamworks API integration;
- an alternative Workshop service;
- dependency downloading or version solving;
- a finalized archive layout;
- production compression choices;
- code mods, scripting runtimes, native plugins, or mod-provided Bevy systems;
- asset inheritance, patching, overriding, or load-order semantics;
- multiplayer synchronization;
- signatures, DRM, or encryption;
- binary compatibility promises for the experimental version-zero format;
- performance work unsupported by measurements.

If an implementation detail forces one of these decisions, introduce a narrow provisional interface, document the assumption, and keep it replaceable. Do not expand the task silently.

## Quality requirements

- Use safe Rust unless an unavoidable reason is documented.
- Prefer explicit error types with useful context over panics or generic strings.
- Keep filesystem discovery, identity, binary encoding, and Bevy integration as separate concerns.
- Do not make the dependency-light core crate depend on the editor or application crate.
- Add API documentation where invariants are not obvious.
- Keep example data tiny and deterministic.
- Run formatting, workspace tests, and Clippy across all targets supported by the repository.
- Preserve unrelated user changes in a dirty worktree.

## Completion report

When finished, report:

1. the files and crates added or changed;
2. the exact filename and identity rules implemented;
3. the experimental container capabilities and limitations;
4. how Bevy dispatch works at a high level;
5. commands used to validate the work and their results;
6. all provisional assumptions and unresolved decisions;
7. the smallest sensible next task, without implementing it.

Do not claim the asset format is production-stable. Success for this task is a small, tested architectural proof that preserves Northstar's package-local identity model and leaves room for future content and mod tooling.
