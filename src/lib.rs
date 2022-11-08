#![allow(clippy::let_and_return)] // This improves readability sometimes
#![forbid(unsafe_code)]

mod asset;
mod runtime;
mod transpile;

use asset::JsScriptLoader;
use bevy::{asset::AssetStage, ecs::schedule::SystemDescriptor, prelude::*};

pub use asset::JsScript;
pub use bevy_ecs_dynamic;
pub use bevy_reflect_fns;
pub use runtime::{
    ops::ecs::types::{
        JsReflectFunctions, JsValueRef, JsValueRefKey, JsValueRefs, ReflectFunctionKey,
    },
    JsRuntimeConfig, JsRuntimeOp, OpContext, OpMap, ScriptInfo,
};
pub use serde_json;
pub use type_map;

use runtime::{JsRuntime, JsRuntimeApi};

#[derive(Default)]
pub struct JsScriptingPlugin {
    /// By default, the plugin will setup script functions corresponding to each [`CoreStage`] to
    /// run at the start of each stage. This disables that behavior so that script stages must be
    /// added manually using [`run_script_fn_system`].
    pub skip_core_stage_setup: bool,
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct ActiveScripts(pub indexmap::IndexSet<Handle<JsScript>, bevy::utils::FixedState>);

impl Plugin for JsScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<JsRuntime>()
            .init_resource::<ActiveScripts>()
            .add_asset::<JsScript>()
            .add_asset_loader(JsScriptLoader)
            .add_system_to_stage(
                AssetStage::AssetEvents,
                load_scripts.after(Assets::<JsScript>::asset_event_system),
            );

        // Call runtime `frame_start()` and `frame_end()` functions at the beginning and end of each frame.
        app.add_system_to_stage(
            CoreStage::First,
            (|world: &mut World| {
                let runtime = world.remove_non_send_resource::<JsRuntime>().unwrap();
                runtime.frame_start(world);
                world.insert_non_send_resource(runtime);
            })
            .at_start(),
        )
        .add_system_to_stage(
            CoreStage::Last,
            (|world: &mut World| {
                let runtime = world.remove_non_send_resource::<JsRuntime>().unwrap();
                runtime.frame_end(world);
                world.insert_non_send_resource(runtime);
            })
            .at_end(),
        );

        if !self.skip_core_stage_setup {
            // Run scripts assocated to each core stage
            for (label, fn_name) in [
                (CoreStage::First, "first"),
                (CoreStage::PreUpdate, "preUpdate"),
                (CoreStage::Update, "update"),
                (CoreStage::PostUpdate, "postUpdate"),
                (CoreStage::Last, "last"),
            ] {
                app.add_system_to_stage(label, run_script_fn_system(fn_name.to_owned()).at_start());
            }
        }
    }
}

/// This returns a system that will run the exported function, `fn_name`, for every script in
/// [`ActiveScripts`].
///
/// This allows you to schedule which stages different script functions will be executed at.
///
/// By default the plugin will run script functions corresponding to Bevy [`CoreStage`]s at the
/// start of each core stage, but this can be disabled by setting
/// [`JsCriptingPlugin::skip_core_stage_setup`] to `true`.
pub fn run_script_fn_system(fn_name: String) -> SystemDescriptor {
    (move |world: &mut World| {
        let active_scripts = world.remove_resource::<ActiveScripts>().unwrap();
        let runtime = world.remove_non_send_resource::<JsRuntime>().unwrap();

        for script in &*active_scripts {
            if runtime.has_loaded(script) {
                runtime.run_script(script, &fn_name, world);
            }
        }

        world.insert_resource(active_scripts);
        world.insert_non_send_resource(runtime);
    })
    .into_descriptor()
}

pub trait AddJsSystem {
    fn add_js_system(&mut self, path: &str) -> &mut Self;
}
impl AddJsSystem for App {
    fn add_js_system(&mut self, path: &str) -> &mut Self {
        let asset_server = self.world.resource::<AssetServer>();
        let handle = asset_server.load(path);

        let mut active = self.world.resource_mut::<ActiveScripts>();
        active.insert(handle);

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
