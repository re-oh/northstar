//! Conventional `tracing` targets ("categories") for Northstar subsystems.
//!
//! `tracing` already lets any call site set an arbitrary `target:`; these
//! constants exist so unrelated crates converge on the same small set of
//! names instead of each inventing its own, which is what makes
//! `RUST_LOG=northstar::assets=debug` (or an equivalent editor log filter
//! UI, later) actually useful. Add a new category here when a subsystem
//! needs one — don't invent one inline at a call site.
//!
//! Usage:
//!
//! ```
//! use northstar_diagnostics::targets;
//! tracing::info!(target: targets::ASSETS, package = "basegame", "mounted package");
//! ```

/// Package mounting, `.nspkg` classification, container decoding, Bevy
/// asset dispatch.
pub const ASSETS: &str = "northstar::assets";

/// Simulation clock, fixed-tick scheduling, pause/scale state.
pub const SIM: &str = "northstar::sim";

/// Editor views, workspace layout, editor-only tooling.
pub const EDITOR: &str = "northstar::editor";

/// Application bootstrap and startup-phase sequencing.
pub const BOOTSTRAP: &str = "northstar::bootstrap";

/// `northstar-dev` and other offline developer tooling.
pub const DEV_TOOLS: &str = "northstar::dev_tools";
