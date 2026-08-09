//! Northstar's simulation clock skeleton.
//!
//! See `docs/simulation-time.md` for the full rationale. In short: this
//! crate exists so gameplay/simulation systems reach for `Res<SimClock>` +
//! `Time<Fixed>` by convention, instead of `Time<Real>`/`Instant::now()`,
//! from the very first system anyone writes — retrofitting determinism
//! after systems already read wall-clock time is much more expensive than
//! establishing the convention now, before there is anything to simulate.

mod clock;

pub use clock::{NorthstarTimePlugin, SimClock, SimClockMode};
