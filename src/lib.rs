#![allow(clippy::let_and_return)] // This improves readability sometimes
#![forbid(unsafe_code)]

mod asset;
mod runtime;
mod transpile;

pub use asset::JsScript;
use asset::JsScriptLoader;
use bevy::{asset::AssetStage, prelude::*};

use bevy_ecs_dynamic::reflect_value_ref::ReflectValueRef;
use bevy_reflect_fns::ReflectFunction;
use runtime::{JsRuntime, JsRuntimeApi};
use slotmap::SlotMap;

pub struct JsScriptingPlugin;

#[derive(Default, Deref, DerefMut)]
pub struct ActiveScripts(pub Vec<Handle<JsScript>>);

slotmap::new_key_type! {
    struct JsValueRefKey;
    struct ReflectFunctionKey;
}

/// Resource that stores [`ReflectValueRef`]s that are accessible to the JS runtime
#[derive(Default, Deref, DerefMut)]
struct JsValueRefs(SlotMap<JsValueRefKey, ReflectValueRef>);

/// Resource that stores [`ReflectFunction`]s that are accessible to the JS runtime
#[derive(Default, Deref, DerefMut)]
struct JsReflectFunctions(SlotMap<ReflectFunctionKey, ReflectFunction>);

impl Plugin for JsScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<JsRuntime>()
            .init_non_send_resource::<JsValueRefs>()
            .init_resource::<JsReflectFunctions>()
            .init_resource::<ActiveScripts>()
            .add_asset::<JsScript>()
            .add_asset_loader(JsScriptLoader)
            .add_system_to_stage(
                AssetStage::AssetEvents,
                load_scripts.after(Assets::<JsScript>::asset_event_system),
            );

        for stage in &[
            CoreStage::First,
            CoreStage::PreUpdate,
            CoreStage::Update,
            CoreStage::PostUpdate,
            CoreStage::Last,
        ] {
            app.add_system_to_stage(
                stage.clone(),
                (move |world: &mut World| {
                    let active_scripts = world.remove_resource::<ActiveScripts>().unwrap();
                    let runtime = world.remove_non_send_resource::<JsRuntime>().unwrap();

                    for script in &*active_scripts {
                        if runtime.has_loaded(script) {
                            runtime.run_script(script, stage, world);
                        }
                    }

                    world.insert_resource(active_scripts);
                    world.insert_non_send_resource(runtime);
                })
                .exclusive_system()
                .at_start(),
            );
        }
    }
}

pub trait AddJsSystem {
    fn add_js_system(&mut self, path: &str) -> &mut Self;
}
impl AddJsSystem for App {
    fn add_js_system(&mut self, path: &str) -> &mut Self {
        let asset_server = self.world.resource::<AssetServer>();
        let handle = asset_server.load(path);

        let mut active = self.world.resource_mut::<ActiveScripts>();
        active.push(handle);

        self
    }
}

/// Helper struct used in [`load_scripts`]
struct ScriptToLoad {
    handle: Handle<JsScript>,
    reload: bool,
}

/// System to finish loading scripts that have had their source-code loaded by the asset server.
fn load_scripts(
    mut scripts_to_load: Local<Vec<ScriptToLoad>>,
    mut events: EventReader<AssetEvent<JsScript>>,
    assets: Res<Assets<JsScript>>,
    engine: NonSendMut<JsRuntime>,
) {
    for event in events.iter() {
        match event {
            AssetEvent::Created { handle } => {
                scripts_to_load.push(ScriptToLoad {
                    handle: handle.clone_weak(),
                    reload: false,
                });
            }
            AssetEvent::Modified { handle } => {
                scripts_to_load.push(ScriptToLoad {
                    handle: handle.clone_weak(),
                    reload: true,
                });
            }
            _ => (),
        }
    }

    // Get the list of scripts we need to try to load
    let mut scripts = Vec::new();
    std::mem::swap(&mut *scripts_to_load, &mut scripts);

    for to_load in scripts {
        // If the script asset has loaded
        if let Some(script) = assets.get(&to_load.handle) {
            // Have the engine load the script
            engine.load_script(&to_load.handle, script, to_load.reload);

        // If the asset hasn't loaded yet
        } else {
            // Add it to the list of scripts to try to load later
            scripts_to_load.push(to_load);
        }
    }
}
