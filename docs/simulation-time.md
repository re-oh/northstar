# Simulation time

`northstar-time` exists to answer one question before any system needs to
ask it: **which clock should I be reading?** Getting this wrong (reading
wall-clock time in simulation code) is the kind of mistake that's nearly
free to avoid on day one and expensive to retrofit once fifty systems
depend on the wrong clock — see the crate's own doc comment for the fuller
version of this argument.

## The four things this distinguishes

- **Rendered frames** — real wall-clock time
  (`bevy::time::Time<Real>`). Interpolation, animation blending, UI
  animation may read this. Simulation logic must not.
- **Fixed simulation ticks** — `bevy::time::Time<Fixed>`, advanced at a
  fixed timestep. This is the *only* clock simulation systems should read.
  Nothing in `northstar-time` reimplements tick bookkeeping — that's
  Bevy's own `Time<Fixed>`/`FixedUpdate` machinery; this crate only adds
  the pause/scale/preview policy layered on top (see below).
- **Paused** — [`SimClockMode::Paused`]. `Time<Fixed>` stops advancing;
  nothing simulation-observable happens. Rendered frames keep happening
  (the editor/UI stays responsive while the sim is paused).
- **Editor preview time** — [`SimClockMode::EditorPreview`]. Reserved for
  the editor scrubbing or previewing a specific moment, distinct from the
  simulation being paused so editor UI and gameplay systems can each ask
  "is *this* what's driving time right now?" without confusing the two. No
  actual preview-time storage/scrubbing is implemented yet.

Time **scale** ([`SimClock::scale`]) applies while
[`SimClockMode::Running`] — `1.0` is real-time, and it's implemented by
setting `Time<Virtual>`'s relative speed (which `Time<Fixed>` derives its
own advancement from), not by scaling deltas by hand in every system.

## Using it

```rust,ignore
fn slow_motion(mut sim_clock: ResMut<SimClock>) {
    sim_clock.set_scale(0.25);
}

fn pause(mut sim_clock: ResMut<SimClock>) {
    sim_clock.set_mode(SimClockMode::Paused);
}

fn my_simulation_system(time: Res<Time<Fixed>>) {
    let dt = time.delta_secs(); // deterministic, respects pause/scale
}
```

`NorthstarTimePlugin` (installed automatically by [`NorthstarPlugin`]) owns
applying `SimClock`'s current mode/scale onto `Time<Virtual>` every frame —
nothing else should call `Time<Virtual>::pause`/`unpause`/`set_relative_speed`
directly, or it'll fight with this system for control the same way Bevy's
own `LogPlugin` fought with `northstar_diagnostics::init_logging` before
`northstar-game` disabled it (see that crate's `main.rs`) — one system
should own a given piece of global state, not several.

## What isn't decided or built yet

- Actual simulation logic — this crate simulates nothing; see its own doc
  comment.
- Editor-preview time storage, scrubbing, or how it interacts with a
  timeline UI (depends on `docs/editor-views.md`'s eventual UI choice).
- Network time synchronization — out of scope until networking exists at
  all.
- Save/restore of `SimClock` state across sessions.

[`SimClockMode::Paused`]: ../crates/northstar-time/src/clock.rs
[`SimClockMode::EditorPreview`]: ../crates/northstar-time/src/clock.rs
[`SimClock::scale`]: ../crates/northstar-time/src/clock.rs
[`NorthstarPlugin`]: ../crates/northstar/src/plugin.rs
