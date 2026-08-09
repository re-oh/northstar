//! Northstar's top-level application bootstrap.
//!
//! [`NorthstarPlugin`] is intentionally small: it installs the foundational
//! plugins (`northstar-bevy`'s asset layer, `northstar-time`'s simulation
//! clock, `northstar-diagnostics`'s startup banner/frame-time diagnostics)
//! and orders four [`NorthstarPhase`] startup sets. It does not choose
//! windowing, does not build a persistent state machine, and does not know
//! anything about gameplay.

mod phase;
mod plugin;

pub use phase::NorthstarPhase;
pub use plugin::NorthstarPlugin;
