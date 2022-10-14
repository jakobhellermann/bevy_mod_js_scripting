use std::{cell::RefCell, path::PathBuf};

use bevy::{prelude::*, utils::HashMap};
use deno_core::{
    error::AnyError, v8, Extension, JsRuntime as DenoJsRuntime, OpState, ResourceId, RuntimeOptions,
};
use type_map::TypeMap;

use super::JsRuntimeApi;
use crate::{
    asset::JsScript,
    runtime::{JsRuntimeConfig, OpContext, OpNames, Ops, ScriptInfo},
};

/// Resource stored in the Deno runtime to give access to the Bevy world
struct WorldResource {
    world: RefCell<World>,
}
impl deno_core::Resource for WorldResource {}
impl WorldResource {
    const RID: ResourceId = 0;
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

impl FromWorld for JsRuntime {
    fn from_world(world: &mut World) -> Self {
        // Collect ops from the runtime config
        let config = world
            .remove_non_send_resource::<JsRuntimeConfig>()
            .unwrap_or_default();
        let custom_ops = config.custom_ops;
        let (ops, op_indexes, op_names) = super::get_ops(custom_ops);

        // Create the Deno extension
        let ext = Extension::builder()
            .ops(vec![op_bevy_mod_js_scripting::decl()])
            // Insert JS from the registered ops
            .js(op_indexes
                .iter()
                .filter_map(|(&op_name, &op_idx)| ops[op_idx].js().map(|js| (op_name, js)))
                .collect())
            .build();

        // Create the runtime
        let mut runtime = DenoJsRuntime::new(RuntimeOptions {
            extensions: vec![ext],
            ..Default::default()
        });

        // Run our initialization script, inserting the op name mapping into the script before running it
        let op_map_json = serde_json::to_string(&op_indexes).unwrap();
        let init_script_src = &include_str!("./js/native_setup.js")
            .replace("__OP_NAME_MAP_PLACEHOLDER__", &op_map_json);
        runtime
            .execute_script("bevy_mod_js_scripting", init_script_src)
            .expect("Init script failed");

        let state = runtime.op_state();
        let mut state_borrow = state.borrow_mut();

        // Insert the ops list, the op name lookup map, and the op state into the Deno state
        state_borrow.put(ops);
        state_borrow.put(op_names);
        state_borrow.put(TypeMap::default()); // op state

        // Insert the world resource
        let rid = state_borrow.resource_table.add(WorldResource {
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
                handle: handle.clone_weak(),
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
            handle: handle.clone_weak(),
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
                CoreStage::PreUpdate => "preUpdate",
                CoreStage::Update => "update",
                CoreStage::PostUpdate => "postUpdate",
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

            let tc_scope = &mut v8::TryCatch::new(scope);
            script_fn.call(tc_scope, output.into(), &[]);
            if let Some(message) = tc_scope.message() {
                let mut stack_trace_message = String::new();
                let stack_trace = message.get_stack_trace(tc_scope).unwrap();
                for i in 0..stack_trace.get_frame_count() {
                    let Some(frame) = stack_trace.get_frame(tc_scope, i) else { continue };
                    let function_name = frame
                        .get_function_name(tc_scope)
                        .map(|name| name.to_rust_string_lossy(tc_scope));
                    let script_name = frame
                        .get_script_name(tc_scope)
                        .map(|name| name.to_rust_string_lossy(tc_scope));
                    stack_trace_message.push_str(&format!(
                        "\n    at {} ({}:{}:{})",
                        function_name.as_deref().unwrap_or("<unknown>"),
                        script_name.as_deref().unwrap_or("<unknown>"),
                        frame.get_line_number(),
                        frame.get_column()
                    ));
                }

                let message = message.get(tc_scope).to_rust_string_lossy(tc_scope);
                let message = message.trim_end_matches("Uncought ");

                error!("{message}{stack_trace_message}");
            }
        });
    }

    fn frame_start(&self, world: &mut World) {
        let this: &mut JsRuntimeInner = &mut self.borrow_mut();
        let op_state = this.runtime.op_state();
        let mut op_state = op_state.borrow_mut();

        with_state(&mut op_state, |op_state, ops: &mut Ops| {
            with_state(op_state, |_, script_op_state: &mut TypeMap| {
                for op in ops {
                    op.frame_start(script_op_state, world);
                }
            });
        });
    }

    fn frame_end(&self, world: &mut World) {
        let this: &mut JsRuntimeInner = &mut self.borrow_mut();

        {
            let op_state = this.runtime.op_state();
            let mut op_state = op_state.borrow_mut();

            with_state(&mut op_state, |op_state, ops: &mut Ops| {
                with_state(op_state, |_, script_op_state: &mut TypeMap| {
                    for op in ops {
                        op.frame_end(script_op_state, world);
                    }
                });
            });
        }
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

/// Core deno op that is used to run any of the core/custom JS ops that are registered
#[deno_core::op]
fn op_bevy_mod_js_scripting(
    state: &mut OpState,
    op_idx: usize,
    args: serde_json::Value,
) -> Result<serde_json::Value, AnyError> {
    with_state(state, |state, custom_op_state| {
        let args = convert_safe_ints(args);
        let script_info = state.borrow::<ScriptInfo>();
        let ops = state.borrow::<Ops>();
        let op_names = state.borrow::<OpNames>();
        let op_name = op_names.get(&op_idx);

        trace!(%op_idx, ?op_name, ?args, "Executing JS Op");

        if let Some(op) = ops.get(op_idx) {
            let world = state
                .resource_table
                .get::<WorldResource>(WorldResource::RID)?;

            let mut world = world.world.borrow_mut();

            let type_registry = world.resource::<AppTypeRegistry>().0.clone();
            let type_registry = type_registry.read();

            let context = OpContext {
                op_state: custom_op_state,
                script_info,
                type_registry: &*type_registry,
            };
            return op.run(context, &mut world, args);
        } else {
            error!("Invalid op index");
        }

        Ok(serde_json::Value::Null)
    })
}

/// Essentially a [`World::resource_scope`] for [`OpState`]
fn with_state<T: 'static, R, F: FnOnce(&mut OpState, &mut T) -> R>(state: &mut OpState, f: F) -> R {
    let mut t = state.take::<T>();

    let r = f(state, &mut t);

    state.put(t);

    r
}

/// Takes a [`serde_json::Value`] and converts all floating point number types that are safe
/// integers, to integers.
///
/// This is important for deserializing numbers to integers, because of the way `serde_json` handles
/// them.
///
/// For example, `serde_json` will not deserialize `1.0` to a `u32` without an error, but it will
/// deserialize `1`. `serde_v8` seems to retun numbers with a decimal point, even when they are
/// valid integers, so this function makes the conversion of safe integers back to integers without
/// a decimal point.
fn convert_safe_ints(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(n) => {
            let max_safe_int = (2u64.pow(53) - 1) as f64;

            serde_json::Value::Number(if let Some(f) = n.as_f64() {
                if f.abs() <= max_safe_int && f.fract() == 0.0 {
                    if f == 0.0 {
                        serde_json::Number::from(0u64)
                    } else if f.is_sign_negative() {
                        serde_json::Number::from(f as i64)
                    } else {
                        serde_json::Number::from(f as u64)
                    }
                } else {
                    n
                }
            } else {
                n
            })
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(|x| convert_safe_ints(x)).collect())
        }
        serde_json::Value::Object(obj) => serde_json::Value::Object(
            obj.into_iter()
                .map(|(k, v)| (k, convert_safe_ints(v)))
                .collect(),
        ),
        other => other,
    }
}
