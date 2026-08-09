# RFC: Coordinates and units

**Status:** proposed — not implemented. Nothing in this repository depends
on the decisions below yet; this document exists so they get made
deliberately, early, once, rather than accreting ad hoc as the first
aircraft/map/physics code gets written. Do not treat any convention here as
binding until it has actually been adopted (updated to "Accepted" with a
date and, ideally, a first real implementation that proves it out).

## Why this can't wait

Coordinate and unit mistakes are unusually expensive to fix retroactively
because they don't fail loudly — they fail as slow drift, jitter, or a
factor-of-3.28 error that only shows up once two systems disagree (imperial
vs. metric was the proximate cause of the Mars Climate Orbiter loss; it is
not a hypothetical failure mode). Every asset schema, save-file format, and
physics equation written against an undocumented convention becomes a
migration once the convention is finally written down. This RFC is the
attempt to write it down first.

## Scope

This covers: coordinate handedness, altitude conventions, local vs. global
coordinates, floating-origin requirements, angles, velocity, mass, and unit
serialization. It does not cover flight-dynamics-model implementation,
terrain representation, or map projection choice — those depend on
decisions here but are separate, larger design efforts.

## 1. Handedness

Bevy is right-handed, Y-up: +X right, +Y up, +Z toward the viewer (i.e.
`-Z` is "forward" by Bevy's own convention for camera-facing objects).
`glam::Vec3`/`Quat`/`Transform` all assume this.

Aviation convention is different and itself not singular:

- **Body frame** (aircraft-relative, used in flight-dynamics equations):
  right-handed, X-forward (nose), Y-right (starboard), Z-down.
- **NED** (North-East-Down, local navigation frame): right-handed,
  X-north, Y-east, Z-down.
- **ECEF** (Earth-Centered-Earth-Fixed, global): right-handed, origin at
  Earth's center, for representing absolute position on/above the globe.

**Recommendation:** don't fight Bevy's Y-up in render/scene-graph code —
every `Transform`, every Bevy physics/rendering integration assumes it, and
diverging there taxes every future dependency on the Bevy ecosystem forever.
Instead, treat NED (and body-frame, for aircraft-local flight-dynamics math)
as the convention at the *simulation/flight-dynamics* boundary, and define
one explicit, tested conversion at the seam between "flight dynamics model"
and "Bevy `Transform`". Do not let both conventions leak into the same
module un-annotated — a function operating in NED should be typed or named
to make that obvious, not left as a bare `Vec3` indistinguishable from a
Bevy-space one.

**Open question:** where exactly that seam lives (inside the eventual
`northstar-flight-dynamics`-equivalent crate, or at its boundary with
whatever ECS component holds aircraft state) is an implementation decision
for whoever builds the flight model, not this RFC.

## 2. Altitude conventions

At least three different "altitude" values matter in an aviation sim, and
they are not interchangeable:

- **MSL** (mean sea level) — what altimeters and ATC communicate in.
- **AGL** (above ground level) — height above the terrain directly below,
  varies with terrain.
- **Ellipsoid height** (e.g. WGS84) — what GPS/GNSS actually measures,
  differs from MSL by the local geoid undulation (up to ~100m depending on
  location).
- **Local/game-space Y** — whatever the floating-origin scheme (§4) uses
  internally; not guaranteed to correspond to any of the above without an
  explicit conversion.

**Recommendation:** never store a bare `f32 altitude` and rely on a comment
or variable name to say which of these it is. At minimum, distinguish them
by type (`AltitudeMsl(f64)`, `AltitudeAgl(f32)`, `EllipsoidHeight(f64)`, or
an enum wrapping a value with its reference) wherever altitude crosses a
module or serialization boundary. Internal, hot-path physics math can still
use plain floats once the reference frame is unambiguous within that
function's scope.

**Open question:** MSL depends on a geoid model, which is data, not free.
Whether Northstar needs true MSL fidelity (a real geoid model) or an
approximation (treat MSL ≈ ellipsoid height, document the error bound) is a
scope decision for whoever builds terrain/navigation, not this RFC — but
whichever is chosen, it needs to be the same choice everywhere altitude is
displayed or compared.

## 3. Local vs. global coordinates, and floating origin

`f32` has ~7 decimal digits of precision. At a position 10,000 km from the
origin (a plausible in-sandbox distance — this is an aviation sandbox, not
a single-airfield sim), a single `f32` unit of precision is already on the
order of a meter; by 100,000 km it's tens of meters. Bevy `Transform`s are
`f32`. Left alone, this produces visible jitter and physically-wrong
collision/physics behavior far from the origin — a well-known problem class
in large-world engines, not specific to Northstar.

**Recommendation:** adopt a two-tier coordinate scheme from the start:

- A **global position** type, stored at higher precision (`f64`, or a
  fixed-point/chunked representation — TBD by whoever implements this) that
  is authoritative for "where is this entity in the world," independent of
  any floating origin.
- A **floating origin**: periodically re-centered on the camera/player (or
  each independently-simulated region, if Northstar ever needs more than
  one active region at once), used to compute the `f32` Bevy `Transform`
  actually handed to rendering/physics each frame as
  `local = global - origin`.

This is a standard pattern (a survey of how other large-world engines solve
it is worth doing before implementation, not worth re-deriving from
scratch) — the point of writing it down here is that **every system that
touches entity position needs to know which of the two representations it
is reading**, and that decision is much cheaper to make before there are a
hundred systems reading `Transform.translation` directly and assuming it's
authoritative world position.

**Open questions:** the concrete global-position type (`f64` vec3 vs.
chunked-integer-plus-offset vs. something else), the re-centering trigger
and threshold, and whether multiple simultaneously-active floating origins
are ever needed (e.g. two aircraft far apart, both needing local precision)
are all implementation decisions for later.

## 4. Angles

**Recommendation:** radians everywhere internally — physics, flight
dynamics, ECS components, save-file math. Degrees exist only at the
editor/UI display boundary (input fields, HUD readouts) and are converted
immediately on entry/exit. This matches Bevy/glam's own convention (`Quat`,
trig functions), so "internal = radians" costs nothing and avoids the
degrees-vs-radians class of bug at every boundary that isn't UI.

Attitude (aircraft orientation) should be represented as a quaternion
(`glam::Quat`, matching `Transform.rotation`) as the source of truth, not
as stored Euler angles — quaternions avoid gimbal lock and compose
correctly; Euler angles (yaw/pitch/roll, aviation's Tait-Bryan ZYX
convention) are a *display and authoring* representation, derived from the
quaternion on demand, not the other way around. Whoever implements the
flight-dynamics model should confirm this holds for the specific equations
being implemented, since some classical flight-dynamics formulations are
written directly in Euler-angle rate terms — if so, that's still fine as an
internal detail of that model, as long as the ECS-visible attitude state
stays quaternion-based at the boundary other systems read.

## 5. Velocity

**Recommendation:** SI base units (meters/second) as the canonical
in-engine representation for both linear and angular velocity, converted to
knots/mph/km·h⁻¹/degrees-per-second only at the UI boundary — same
reasoning as angles.

Distinguish **frame of reference** explicitly wherever velocity is stored
or passed between systems: body-frame velocity (relative to the aircraft's
own orientation — what a flight-dynamics model typically integrates),
world/NED-frame velocity, and (aviation-specific) **airspeed vs.
groundspeed** — airspeed depends on the surrounding air mass's motion
(wind), groundspeed doesn't. These are not the same number and conflating
them is a correctness bug, not a units bug, but it belongs in this
document because it's the same discipline: name/type velocity values by
which of these they are rather than passing a bare `Vec3`/`f32` and relying
on context.

## 6. Mass

**Recommendation:** SI throughout — kilograms for mass, kg·m² for moments
of inertia, meters for center-of-gravity offset. No pounds, no slugs
in-engine, ever, even though real-world aviation references (POHs, weight
and balance sheets) are frequently published in imperial units — convert at
the content-authoring/import boundary, not in simulation code.

Center of gravity and moment-of-inertia tensor representation (single point
+ diagonal inertia vs. full tensor, whether CG shifts with fuel burn are
modeled) are flight-dynamics-model implementation decisions, out of scope
here — the only claim this RFC makes is: whatever the representation, its
units are SI and that's enforced at the schema/import boundary, not left to
convention.

## 7. Unit serialization

Once `.nspkg` content actually carries numeric physical quantities (aircraft
performance data, mass properties, map elevation data, ...), the
container/schema layer (see `docs/assets.md`) needs those quantities to be
self-evidently unambiguous — a `f32` field named `mass` in a RON file is
exactly the kind of thing that causes the failure mode this document opened
with, and mod content is exactly the place where "I authored this in the
wrong unit" is most likely to happen and least likely to be caught before
runtime.

**Recommendation:** at data-schema boundaries (asset files, save files, any
mod-authorable data), physical quantities should be represented with their
unit either encoded in the field name/type (`mass_kg: f32`) or via a small
set of newtype wrappers with a fixed canonical unit
(`Kilograms(f32)`, `Meters(f64)`, `Radians(f32)`, `MetersPerSecond(f32)`)
that (de)serialize predictably and are hard to accidentally mix up with a
bare number. Which of those two mechanisms (naming convention vs. newtypes)
is worth the ergonomics cost is a decision for whoever designs the first
real content schema that needs it (see the asset foundation brief's
"real map/mission/prefab/... schemas" non-goal — this RFC deliberately
doesn't reach into that). Internal hot-path math is not required to use the
newtypes if they cost measurable performance — but the *boundary* (parsing
into and serializing out of them) should, so a unit mistake is caught at
content-load time, not silently propagated into physics.

## Non-goals of this RFC

- Choosing a specific map projection or terrain representation.
- Specifying the flight-dynamics model's actual equations.
- Choosing the concrete global-position type for floating origin (§3) —
  flagged as an open question, not decided here.
- Deciding whether/how MSL vs. ellipsoid height fidelity matters (§2) —
  same.

## Adoption

This RFC becomes binding once accepted (updated status, date) — most
usefully at the point someone is about to write the first flight-dynamics
math, the first floating-origin-sensitive system, or the first
physical-quantity-carrying asset schema, so the decision is validated
against a real use rather than accepted in the abstract.
