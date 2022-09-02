use std::time::Duration;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::{
    app::ScheduleRunnerSettings,
    asset::{AssetPlugin, AssetServerSettings},
};
use bevy_mod_js_scripting::{AddJsSystem, JsScriptingPlugin};
use bevy_reflect::TypeRegistryArc;
use bevy_reflect_fns::ReflectMethods;

fn main() {
    App::new()
        .insert_resource(AssetServerSettings {
            watch_for_changes: true,
            ..default()
        })
        .insert_resource(ScheduleRunnerSettings::run_loop(Duration::from_millis(200)))
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

fn setup(mut commands: Commands, type_registry: ResMut<TypeRegistryArc>) {
    let mut type_registry = type_registry.write();
    type_registry.register::<Transform>();
    type_registry
        .get_mut(std::any::TypeId::of::<Vec3>())
        .unwrap()
        .insert(ReflectMethods::from_methods([
            (
                "normalize",
                bevy_reflect_fns::reflect_function!(Vec3::normalize, (Vec3)),
            ),
            (
                "lerp",
                bevy_reflect_fns::reflect_function!(Vec3::lerp, (Vec3, Vec3, f32)),
            ),
        ]));

    commands.spawn_bundle((
        TestComponent {
            value: "component 1".into(),
            number: 28,
        },
        Transform::from_xyz(1.0, 0.0, 0.0),
    ));
    commands.spawn_bundle((
        TestComponent {
            value: "component 2".into(),
            number: 79,
        },
        Marker,
        Transform::from_xyz(12.0, 4.0, 3.0),
    ));

    commands.insert_resource(TestResource {
        transform: Transform::from_xyz(10.0, 1.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    });
}
