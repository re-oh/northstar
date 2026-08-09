//! Logging, build/version info, startup diagnostics, and panic reporting
//! for Northstar.
//!
//! This crate deliberately does *not* try to be the one place that also
//! defines error types — see `docs/errors.md`. It owns exactly: how things
//! get logged (categories, filtering), what gets logged at startup
//! (build info, optional frame-time diagnostics), and how panics are
//! reported. Actionable error messages themselves live where the error
//! happens (e.g. `northstar_bevy::NspkgLoadError`).
//!
//! Typical wiring, in a binary's `main`, before any `App` is constructed:
//!
//! ```no_run
//! northstar_diagnostics::install_panic_hook();
//! northstar_diagnostics::init_logging();
//! ```
//!
//! then add [`NorthstarDiagnosticsPlugin`] to the `App` like any other
//! plugin.

mod build_info;
mod logging;
mod panic_hook;
mod plugin;

pub mod targets;

pub use build_info::BuildInfo;
pub use logging::init_logging;
pub use panic_hook::install as install_panic_hook;
pub use plugin::NorthstarDiagnosticsPlugin;
