use std::time::Duration;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::{
    app::ScheduleRunnerSettings,
    asset::{AssetPlugin, AssetServerSettings},
};
use bevy_mod_js_scripting::{AddJsSystem, JsScriptingPlugin};

fn main() {
    App::new()
        .insert_resource(AssetServerSettings {
            watch_for_changes: true,
            ..default()
        })
        .insert_resource(ScheduleRunnerSettings::run_loop(Duration::from_millis(
            1_000,
        )))
        .add_plugins(MinimalPlugins)
        .add_plugin(LogPlugin)
        .add_plugin(AssetPlugin)
        .add_plugin(JsScriptingPlugin)
        .add_startup_system(setup)
        .add_js_system("scripts/headless.ts")
        .register_type::<TestComponent>()
        .run();
}

#[derive(Component, Reflect)]
struct TestComponent {
    value: String,
    number: u8,
}

#[derive(Component, Reflect)]
struct TestResource {
    transform: Transform,
}

#[derive(Component)]
struct Marker;

fn setup(mut commands: Commands) {
    commands.spawn_bundle((TestComponent {
        value: "component 1".into(),
        number: 28,
    },));
    commands.spawn_bundle((
        TestComponent {
            value: "component 2".into(),
            number: 79,
        },
        Marker,
    ));

    commands.insert_resource(TestResource {
        transform: Transform::from_xyz(10.0, 1.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    });
}
