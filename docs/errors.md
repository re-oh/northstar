# Error architecture

Northstar's error handling follows one rule: **errors are typed per
boundary, not universal.** There is no `NorthstarError` enum that every
fallible function in the workspace returns. This document says why, and
what to do instead when adding a new fallible operation.

## Why not one universal error type

A single `NorthstarError` covering package identity, filename parsing,
container decoding, Bevy asset loading, editor I/O, and eventually
simulation and networking either:

- becomes a huge enum every crate depends on, coupling unrelated concerns
  (a filename-parsing bug fix now touches the same type as a networking
  error) and making `northstar-core` — which must stay dependency-light and
  Bevy-free — depend on nothing, forcing the universal type to live
  somewhere and everything else to import it back down; or
- becomes a thin wrapper around `Box<dyn Error>` / `anyhow::Error`, which
  throws away the ability to `match` on what actually went wrong — exactly
  the capability [`northstar_core::filename::ClassifyError`] and
  [`northstar_core::container::ContainerError`] exist to preserve (see
  `docs/architecture.md`'s filename-classification section: "the classifier
  never guesses or silently normalizes an identity" — a caller needs to be
  able to tell *which* malformation occurred, not just that one did).

Neither serves callers well. What already works in this codebase (see
`northstar-core::filename::ClassifyError`, `northstar-core::container::ContainerError`,
`northstar-bevy::loader::NspkgLoadError`) is: one focused error enum per
crate/module boundary, using [`thiserror`](https://docs.rs/thiserror) for
the boilerplate, with variants that carry the context a caller actually
needs to act on or report.

## The convention

1. **One error type per boundary**, not per function. `ClassifyError`
   covers everything that can go wrong classifying a filename;
   `ContainerError` covers everything that can go wrong parsing/encoding a
   container. A new subsystem (simulation, networking, editor I/O) gets its
   own error type(s) when it has its own boundary — don't retrofit an
   existing one to cover unrelated failures just because it's convenient.

2. **Wrap, don't erase, at a boundary crossing.** When one layer's
   operation fails inside another layer's operation, wrap the source error
   as a field (`#[source]` for `std::error::Error` sources — see the
   `#[source]` vs. plain-field note below) rather than converting it to a
   string immediately. `northstar_bevy::NspkgLoadError::Container` wraps a
   `northstar_core::ContainerError` this way; the caller can still match on
   the original variant if it needs to, and `Display` still produces one
   readable message via the `{0}`/named-field interpolation chain.

3. **`#[source]` needs `std::error::Error`; not everything does.** Bevy's
   `BevyError` does *not* implement `std::error::Error` (it's `Box`-like,
   not itself a source), so `northstar_bevy::NspkgLoadError::Decode` stores
   it as a plain field and interpolates it via `Display` in the `#[error]`
   message instead of marking it `#[source]`. Don't force `#[source]` onto
   a field whose type doesn't support it — a plain field with an
   informative `#[error(...)]` message is fine.

4. **Explicit errors over panics for anything touching untrusted input** —
   filenames, container bytes, network data, mod-provided data of any kind.
   Panics are reserved for **programmer errors**: a static misconfiguration
   that should have been caught at app-setup time and can't be meaningfully
   recovered from at the call site. `northstar_bevy::registry` panicking on
   a conflicting `register_nspkg_asset` call is the model case — that's a
   bug in the calling code, discovered once, at startup, not a runtime
   condition to route around. See `northstar-core`'s crate docs and
   `AGENTS.md`'s ground rules for the "no panics on untrusted input" side of
   this.

5. **Keep filesystem discovery, identity, binary encoding, and Bevy
   integration as separate concerns** (this line is also in `AGENTS.md`) —
   which in practice means their error types stay separate too. If a
   function's error type is reaching across those boundaries (e.g. a
   filename-classification function returning a variant about file I/O),
   that's a sign the function itself is doing too much, not that the error
   enum needs a new variant.

## Reporting helpers

`northstar-diagnostics` owns *how* things get logged (categories, startup
banner, panic hook — see its crate docs) but deliberately does not define
error types itself, and does not provide a generic "report any error"
helper. Each error type's own `Display` (via `thiserror`'s `#[error(...)]`)
is the reporting helper — write the message to be useful on its own, at the
point it's logged or shown, rather than relying on a caller to add context
a generic reporter can't know. If a genuinely repeated reporting pattern
emerges (e.g. "log this error at its crate's `tracing` target and also
surface it in the editor status bar"), add a small, focused helper for
*that* pattern when it appears twice, not preemptively.

## When you're adding a new fallible operation

- Does an error type already exist for this boundary? Add a variant to it.
- Is this a genuinely new boundary? Define a new `thiserror`-derived enum
  next to the code it covers, named `<Thing>Error` (`ClassifyError`,
  `ContainerError`, `NspkgLoadError` — not `Error` alone, which collides
  across modules, and not `NorthstarError`).
- Is the failure a static configuration bug rather than a runtime
  condition? Panic, with a message that says what was misconfigured and
  how to fix it — don't invent an error type nobody is expected to handle.
