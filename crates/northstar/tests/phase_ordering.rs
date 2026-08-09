//! Proves `NorthstarPhase` ordering is real scheduling behavior, not just a
//! declared enum — systems placed in each phase must observe the phases
//! before them having already run.

use bevy::app::{App, Startup};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::ResMut;

use northstar::NorthstarPhase;
use northstar_bevy::PackageCatalog;

#[derive(Resource, Default)]
struct PhaseLog(Vec<&'static str>);

fn record(name: &'static str) -> impl Fn(ResMut<PhaseLog>) {
    move |mut log: ResMut<PhaseLog>| log.0.push(name)
}

#[test]
fn phases_run_in_declared_order() {
    let temp_root =
        std::env::temp_dir().join(format!("northstar-phase-order-test-{}", std::process::id()));
    std::fs::create_dir_all(&temp_root).unwrap();

    let mut app = App::new();
    app.add_plugins((
        bevy::MinimalPlugins,
        northstar::NorthstarPlugin::new(PackageCatalog::loose_directory(&temp_root)),
        bevy::asset::AssetPlugin::default(),
    ));
    app.init_resource::<PhaseLog>();
    app.add_systems(
        Startup,
        (
            record("bootstrap").in_set(NorthstarPhase::Bootstrap),
            record("mount_packages").in_set(NorthstarPhase::MountPackages),
            record("load_assets").in_set(NorthstarPhase::LoadAssets),
            record("app_startup").in_set(NorthstarPhase::AppStartup),
        ),
    );

    app.update();

    let log = app.world().resource::<PhaseLog>();
    assert_eq!(
        log.0,
        vec!["bootstrap", "mount_packages", "load_assets", "app_startup"]
    );

    let _ = std::fs::remove_dir_all(&temp_root);
}
