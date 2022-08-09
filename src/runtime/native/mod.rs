mod ecs;
mod log;

use std::{cell::RefCell, path::PathBuf};

use bevy::{prelude::*, utils::HashMap};
use deno_core::{
    include_js_files, v8, Extension, JsRuntime as DenoJsRuntime, ResourceId, RuntimeOptions,
};

use super::JsRuntimeApi;
use crate::asset::JsScript;

/// Resource stored in the Deno runtime to give access to the Bevy world
struct WorldResource {
    world: RefCell<World>,
}
impl deno_core::Resource for WorldResource {}
impl WorldResource {
    const RID: ResourceId = 0;
}

/// Info about the currently executing script, stored in the Deno op_state for use in ops such as
/// logging.
struct ScriptInfo {
    path: PathBuf,
}

/// The [`JsRuntimeApi`] implementation for native platforms.
#[derive(Deref, DerefMut)]
pub struct JsRuntime(RefCell<JsRuntimeInner>);

pub struct JsRuntimeInner {
    scripts: HashMap<Handle<JsScript>, LoadedScriptData>,
    runtime: deno_core::JsRuntime,
}

struct LoadedScriptData {
    output: v8::Global<v8::Value>,
    path: PathBuf,
}

impl Default for JsRuntime {
    fn default() -> Self {
        let mut runtime = DenoJsRuntime::new(RuntimeOptions {
            extensions: vec![
                ecs::extension(),
                log::extension(),
                Extension::builder()
                    .js(include_js_files!(prefix "bevy", "./native_setup.js",))
                    .build(),
            ],
            ..Default::default()
        });

        let state = runtime.op_state();
        let rid = state.borrow_mut().resource_table.add(WorldResource {
            world: RefCell::new(World::default()),
        });
        assert_eq!(rid, WorldResource::RID);

        Self(RefCell::new(JsRuntimeInner {
            scripts: Default::default(),
            runtime,
        }))
    }
}

impl JsRuntimeApi for JsRuntime {
    fn load_script(&self, handle: &Handle<JsScript>, script: &JsScript, reload: bool) {
        let mut this = self.borrow_mut();
        let already_loaded = this.scripts.contains_key(handle);

        // Skip if already loaded and we aren't intentionally reloading
        if already_loaded && !reload {
            return;
        }

        // Helper to load script
        let mut load_script = || {
            // Get the script source code
            let code = &script.source;

            // Wrap the script in a closure
            let code = format!(
                r#"
                    "strict_mode";
                    
                    ((window) => {{
                        {code}
                    }})(globalThis)
                "#,
            );

            // Make script info available to the runtime
            this.runtime.op_state().borrow_mut().put(ScriptInfo {
                path: script.path.clone(),
            });

            // Run the script and get it's output
            let output = this
                .runtime
                .execute_script(&script.path.to_string_lossy(), &code)?;

            debug!(?script.path, "Loaded script");

            // Store the module's exported namespace in the script map
            this.scripts.insert(
                handle.clone_weak(),
                LoadedScriptData {
                    output,
                    path: script.path.clone(),
                },
            );

            Ok::<_, anyhow::Error>(())
        };

        // Load script or report errors
        if let Err(e) = load_script() {
            error!("Error running script: {}", e);
        }
    }

    fn has_loaded(&self, handle: &Handle<JsScript>) -> bool {
        self.borrow().scripts.contains_key(handle)
    }

    fn run_script(&self, handle: &Handle<JsScript>, stage: &CoreStage, world: &mut World) {
        let mut this = self.borrow_mut();
        let JsRuntimeInner { scripts, runtime } = &mut *this;

        // Get the script output
        let script = if let Some(script) = scripts.get(handle) {
            script
        } else {
            return;
        };

        // Make script info available to the runtime
        runtime.op_state().borrow_mut().put(ScriptInfo {
            path: script.path.clone(),
        });

        with_world(world, runtime, |runtime| {
            let scope = &mut runtime.handle_scope();
            let output = v8::Local::new(scope, &script.output);

            // Make sure that script output was an object
            let output = if let Ok(value) = v8::Local::<v8::Object>::try_from(output) {
                value
            } else {
                warn!(?script.path, "Script init() did not return an object. Skipping.");
                return;
            };

            // Figure out which function to call on the exported object
            let fn_name_str = match stage {
                CoreStage::First => "first",
                CoreStage::PreUpdate => "pre_update",
                CoreStage::Update => "update",
                CoreStage::PostUpdate => "post_update",
                CoreStage::Last => "last",
            };

            // Get a javascript value for the name of the function to call
            let fn_name = v8::String::new_from_utf8(
                scope,
                fn_name_str.as_bytes(),
                v8::NewStringType::Internalized,
            )
            .unwrap();

            // Get get the named function from the object
            let script_fn = if let Some(script_fn) = output.get(scope, fn_name.into()) {
                script_fn
            } else {
                warn!(?script.path, "Getting function named `{}` on script init() value failed. Skipping.", fn_name_str);
                return;
            };

            // Make sure the value is a function
            let script_fn = if let Ok(value) = v8::Local::<v8::Function>::try_from(script_fn) {
                value
            } else {
                // It is valid to not have a function for a script stage so we don't print a warning if
                // the function isn't found.
                return;
            };

            script_fn.call(scope, output.into(), &[]);
        });
    }
}

/// Helper to insert the Bevy world into into the deno resource map while a closure is executed, and
/// remove the world when the closure finishes.
pub fn with_world<T>(
    world: &mut World,
    runtime: &mut DenoJsRuntime,
    f: impl FnOnce(&mut DenoJsRuntime) -> T,
) -> T {
    let resource = runtime
        .op_state()
        .borrow_mut()
        .resource_table
        .get::<WorldResource>(WorldResource::RID)
        .unwrap();
    std::mem::swap(world, &mut *resource.world.borrow_mut());

    let ret = f(runtime);

    let resource = runtime
        .op_state()
        .borrow_mut()
        .resource_table
        .get::<WorldResource>(WorldResource::RID)
        .unwrap();
    std::mem::swap(world, &mut *resource.world.borrow_mut());

    ret
}
