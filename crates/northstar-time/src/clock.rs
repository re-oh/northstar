use bevy::app::{App, Plugin, PreUpdate};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};
use bevy::time::{Time, Virtual};

use northstar_diagnostics::targets;

/// What's currently driving the simulation clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SimClockMode {
    /// The simulation advances at [`SimClock::scale`] × real time.
    #[default]
    Running,
    /// The simulation does not advance. Rendered frames still happen —
    /// only [`bevy::time::Time<Fixed>`] stops ticking.
    Paused,
    /// The editor is scrubbing or previewing a specific moment rather than
    /// running the live simulation. Distinct from `Paused` so editor UI and
    /// gameplay systems can each ask "are *you* driving time right now?"
    /// without one being mistaken for the other.
    EditorPreview,
}

/// The single authority for "what time is it, for simulation purposes".
///
/// Nothing here simulates anything — this crate only establishes the
/// distinction gameplay/simulation code needs from day one to stay
/// deterministic:
///
/// - **rendered frames** — real wall-clock time, [`bevy::time::Time<Real>`].
///   Interpolation, animation blending, and UI may read this; simulation
///   logic must not.
/// - **fixed simulation ticks** — [`bevy::time::Time<Fixed>`], advanced at a
///   fixed timestep and the *only* clock simulation systems should read.
///   `SimClock` governs whether it advances at all (paused) and how fast
///   (scale) by driving Bevy's own `Time<Virtual>`, which `Time<Fixed>` is
///   derived from — this crate doesn't reimplement tick bookkeeping, only
///   the pause/scale/preview *policy* on top of it.
/// - **paused** — [`SimClockMode::Paused`]: `Time<Fixed>` stops advancing;
///   nothing simulation-observable happens.
/// - **editor preview time** — [`SimClockMode::EditorPreview`]: reserved
///   for the editor scrubbing a specific moment without that being
///   confused with the live simulation being paused. No preview-time
///   storage is implemented yet — see `docs/simulation-time.md`.
/// - **time scaling** — [`SimClock::scale`]: applied as `Time<Virtual>`'s
///   relative speed while [`SimClockMode::Running`].
#[derive(Resource, Debug, Clone, Copy)]
pub struct SimClock {
    mode: SimClockMode,
    scale: f32,
}

impl Default for SimClock {
    fn default() -> Self {
        Self {
            mode: SimClockMode::default(),
            scale: 1.0,
        }
    }
}

impl SimClock {
    pub fn mode(&self) -> SimClockMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: SimClockMode) {
        self.mode = mode;
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.mode, SimClockMode::Paused)
    }

    /// Simulation speed multiplier applied while
    /// [`SimClockMode::Running`]. `1.0` is real-time; `0.0` is equivalent
    /// to (but distinct in intent from) [`SimClockMode::Paused`].
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Sets the time scale. Negative values are clamped to `0.0` — time
    /// does not run backwards.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.0);
    }
}

/// Installs [`SimClock`] and the system that applies it to Bevy's
/// `Time<Virtual>` (and therefore `Time<Fixed>`) every frame.
pub struct NorthstarTimePlugin;

impl Plugin for NorthstarTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimClock>()
            .add_systems(PreUpdate, apply_sim_clock_to_virtual_time);
    }
}

fn apply_sim_clock_to_virtual_time(
    sim_clock: Res<SimClock>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    match sim_clock.mode() {
        SimClockMode::Running => {
            if virtual_time.is_paused() {
                virtual_time.unpause();
                tracing::debug!(target: targets::SIM, "simulation resumed");
            }
            let scale = sim_clock.scale();
            if virtual_time.relative_speed() != scale {
                virtual_time.set_relative_speed(scale);
            }
        }
        SimClockMode::Paused | SimClockMode::EditorPreview => {
            if !virtual_time.is_paused() {
                virtual_time.pause();
                tracing::debug!(target: targets::SIM, mode = ?sim_clock.mode(), "simulation stopped advancing");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use bevy::time::{Fixed, Time};

    use super::*;

    #[test]
    fn paused_mode_pauses_virtual_time() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_plugins(NorthstarTimePlugin);

        app.world_mut()
            .resource_mut::<SimClock>()
            .set_mode(SimClockMode::Paused);
        app.update();

        assert!(app.world().resource::<Time<Virtual>>().is_paused());
    }

    #[test]
    fn running_mode_applies_scale() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin);
        app.add_plugins(NorthstarTimePlugin);

        app.world_mut().resource_mut::<SimClock>().set_scale(2.0);
        app.update();

        assert_eq!(
            app.world().resource::<Time<Virtual>>().relative_speed(),
            2.0
        );
        // Time<Fixed> exists and is distinct from wall-clock Time<Real> —
        // this is the type-level distinction the whole crate exists for.
        let _fixed_is_a_real_resource = app.world().resource::<Time<Fixed>>();
    }

    #[test]
    fn negative_scale_is_clamped_to_zero() {
        let mut clock = SimClock::default();
        clock.set_scale(-5.0);
        assert_eq!(clock.scale(), 0.0);
    }
}
