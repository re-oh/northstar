use bevy::app::{App, Plugin, Startup};
use bevy::ecs::schedule::IntoScheduleConfigs;

use northstar_bevy::{NorthstarAssetPlugin, PackageCatalog};
use northstar_diagnostics::{NorthstarDiagnosticsPlugin, targets};
use northstar_time::NorthstarTimePlugin;

use crate::phase::NorthstarPhase;

/// Installs Northstar's foundational plugins and establishes the ordered
/// [`NorthstarPhase`] startup sets. Nothing else — no rendering, no
/// windowing, no gameplay systems. What binary adds this and with which
/// other Bevy plugins (a windowed `northstar-game`, a headless
/// `NorthstarTestApp`, an editor) is deliberately not this crate's
/// decision.
///
/// **Must be added before `AssetPlugin`** (typically part of
/// `DefaultPlugins`/`MinimalPlugins` + `AssetPlugin`), because it installs
/// [`NorthstarAssetPlugin`], which has that same requirement — see its own
/// doc comment.
pub struct NorthstarPlugin {
    catalog: PackageCatalog,
}

impl NorthstarPlugin {
    pub fn new(catalog: PackageCatalog) -> Self {
        Self { catalog }
    }
}

impl Plugin for NorthstarPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            NorthstarAssetPlugin::new(self.catalog.clone()),
            NorthstarTimePlugin,
            NorthstarDiagnosticsPlugin::default(),
        ));

        app.configure_sets(
            Startup,
            (
                NorthstarPhase::Bootstrap,
                NorthstarPhase::MountPackages,
                NorthstarPhase::LoadAssets,
                NorthstarPhase::AppStartup,
            )
                .chain(),
        );

        for phase in NorthstarPhase::ORDER {
            app.add_systems(
                Startup,
                (move || {
                    tracing::debug!(target: targets::BOOTSTRAP, phase = ?phase, "entering startup phase");
                })
                .in_set(phase),
            );
        }
    }
}
