mod asset;
mod dynamic_query;
mod runtime;
#[cfg(feature = "typescript")]
mod ts_to_js;

use asset::{JsScript, JsScriptLoader};
use bevy::utils::HashMap;
use bevy::{asset::AssetStage, prelude::*};
use deno_core::{v8, JsRuntime};
use runtime::create_runtime;

pub struct JsScriptingPlugin;

enum RuntimeStatus {
    FailedToLoad,
    Loaded,
}
struct LoadedRuntime {
    runtime: JsRuntime,
    status: RuntimeStatus,
}

#[derive(Default)]
pub struct ActiveScripts {
    runtimes: HashMap<Handle<JsScript>, LoadedRuntime>,
    by_stage: HashMap<CoreStage, Vec<Handle<JsScript>>>,
}

impl Plugin for JsScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(ActiveScripts::default());
        app.add_asset::<JsScript>().add_asset_loader(JsScriptLoader);

        app.add_system_to_stage(
            AssetStage::AssetEvents,
            load_scripts.after(Assets::<JsScript>::asset_event_system),
        );

        for stage in [
            CoreStage::First,
            CoreStage::PreUpdate,
            CoreStage::Update,
            CoreStage::PostUpdate,
            CoreStage::Last,
        ] {
            app.add_system_to_stage(
                stage.clone(),
                (move |world: &mut World| {
                    let mut active = world.remove_non_send_resource::<ActiveScripts>().unwrap();
                    world.resource_scope(|world, assets: Mut<Assets<JsScript>>| {
                        let in_stage = active
                            .by_stage
                            .entry(stage.clone())
                            .or_default()
                            .iter()
                            .filter(|&handle| assets.contains(handle));
                        for handle in in_stage {
                            if let Some(runtime) = active.runtimes.get_mut(handle) {
                                if let RuntimeStatus::Loaded = runtime.status {
                                    run_js_script(world, &mut runtime.runtime);
                                }
                            }
                        }
                    });

                    world.insert_non_send_resource(active);
                })
                .exclusive_system()
                .at_start(),
            );
        }
    }
}

pub trait AddJsSystem {
    fn add_js_system_to_stage(&mut self, stage: CoreStage, path: &str) -> &mut Self;
    fn add_js_system(&mut self, path: &str) -> &mut Self {
        self.add_js_system_to_stage(CoreStage::Update, path)
    }
}
impl AddJsSystem for App {
    fn add_js_system_to_stage(&mut self, stage: CoreStage, path: &str) -> &mut Self {
        let asset_server = self.world.resource::<AssetServer>();
        let handle = asset_server.load(path);

        let mut active = self.world.non_send_resource_mut::<ActiveScripts>();
        active.by_stage.entry(stage).or_default().push(handle);

        self
    }
}

fn load_scripts(
    mut events: EventReader<AssetEvent<JsScript>>,
    mut active_scripts: NonSendMut<ActiveScripts>,
    assets: Res<Assets<JsScript>>,
) {
    for event in events.iter() {
        match event {
            AssetEvent::Created { handle } | AssetEvent::Modified { handle } => {
                let js_script = assets.get(handle).unwrap();

                let mut runtime = create_runtime(js_script.path.clone());
                let name = js_script.path.display().to_string();

                let status = match runtime.execute_script(&name, &js_script.source) {
                    Ok(_) => RuntimeStatus::Loaded,
                    Err(e) => {
                        warn!("failed to load {name}: {e}");
                        RuntimeStatus::FailedToLoad
                    }
                };

                active_scripts
                    .runtimes
                    .insert(handle.clone_weak(), LoadedRuntime { runtime, status });
            }
            AssetEvent::Removed { .. } => {}
        }
    }
}

fn run_js_script(world: &mut World, runtime: &mut JsRuntime) {
    let res = runtime::with_world(world, runtime, |runtime| {
        let context = runtime.global_context();
        let context = context.open(runtime.v8_isolate());
        let scope = &mut runtime.handle_scope();
        let global = context.global(scope);
        let run_str = v8::String::new(scope, "run").unwrap();
        let run_fn = global
            .get(scope, run_str.into())
            .ok_or_else(|| anyhow::anyhow!("script has no `run` function"))?;
        let run_fn = v8::Local::<v8::Function>::try_from(run_fn)
            .map_err(|_| anyhow::anyhow!("`run` should be a function"))?;

        let undefined = v8::undefined(scope);
        run_fn.call(scope, undefined.into(), &[undefined.into()]);

        Ok::<_, anyhow::Error>(())
    });

    if let Err(e) = res {
        warn!("script failed to run: {:?}", e);
    }
}
