use bevy::ecs::schedule::SystemSet;

/// The ordered phases Northstar's own `Startup` systems run in.
///
/// This is deliberately just an ordering of [`bevy::ecs::schedule::SystemSet`]s,
/// not a persistent state machine — there is no `NorthstarState` resource
/// tracking "which phase are we in right now" after startup finishes, and
/// no transition logic to get that wrong. Once `Startup` has run, these
/// sets have simply already executed in order; that's the entire
/// mechanism. Other crates hook in with
/// `app.add_systems(Startup, my_system.in_set(NorthstarPhase::MountPackages))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum NorthstarPhase {
    /// Diagnostics (logging, panic hook, build-info banner) are up by the
    /// time this set runs. Nothing currently populates it; it exists as an
    /// explicit landing point for future one-time bootstrap work that
    /// isn't package- or asset-related.
    Bootstrap,
    /// Mount package catalogs (see `northstar_bevy::PackageCatalog`). No
    /// systems populate this yet — see `docs/assets.md`.
    MountPackages,
    /// Kick off asset loads that gameplay startup depends on.
    LoadAssets,
    /// Application/game-specific startup: spawning the initial scene,
    /// entering a menu, etc. Runs last, after mounting and asset loads
    /// have at least been kicked off.
    AppStartup,
}

impl NorthstarPhase {
    /// All phases, in their fixed execution order.
    pub const ORDER: [NorthstarPhase; 4] = [
        NorthstarPhase::Bootstrap,
        NorthstarPhase::MountPackages,
        NorthstarPhase::LoadAssets,
        NorthstarPhase::AppStartup,
    ];
}
