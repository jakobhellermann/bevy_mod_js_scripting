use std::path::PathBuf;
use std::rc::Rc;

use bevy::prelude::*;
use bevy::utils::HashMap;
use serde::Serialize;
use type_map::TypeMap;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_mutex::{Mutex, MutexRef};

use super::{get_ops, JsRuntimeApi, JsRuntimeConfig, OpNames, Ops};
use crate::asset::JsScript;
use crate::runtime::{OpContext, ScriptInfo};

/// Panic message when a mutex lock fails
const LOCK_SHOULD_NOT_FAIL: &str =
    "Mutex lock should not fail because there should be no concurrent access";

#[wasm_bindgen]
struct BevyModJsScripting {
    state: Rc<Mutex<JsRuntimeState>>,
}

impl BevyModJsScripting {
    /// Lock the state and panic if the lock cannot be obtained immediately.
    fn state(&self) -> MutexRef<JsRuntimeState> {
        self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL)
    }
}

#[wasm_bindgen]
impl BevyModJsScripting {
    pub fn op_sync(&self, op_idx: usize, args: JsValue) -> Result<JsValue, JsValue> {
        let JsRuntimeState {
            script_info,
            op_state,
            ops,
            op_names,
            world,
        } = &mut *self.state();
        let op_name = op_names.get(&op_idx);
        let args: serde_json::Value = serde_wasm_bindgen::from_value(args).to_js_error()?;
        trace!(%op_idx, ?op_name, ?args, "Executing JS OP..");

        if let Some(op) = ops.get(op_idx) {
            let context = OpContext {
                op_state,
                script_info,
            };
            let result = op
                .run(context, world, args)
                .map_err(|e| format!("Op Error: {e}"))
                .to_js_error()?;

            let serializer = &serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
            return Ok(result.serialize(serializer)?);
        } else {
            error!("Invalid op index");
        }

        Ok(JsValue::NULL)
    }
}

struct JsRuntimeState {
    script_info: ScriptInfo,
    op_state: TypeMap,
    ops: Ops,
    op_names: OpNames,
    world: World,
}

#[wasm_bindgen(module = "/src/runtime/js/wasm_setup.js")]
extern "C" {
    fn setup_js_globals(bevy_mod_js_scripting: BevyModJsScripting, op_name_map: &str);
}

pub struct JsRuntime {
    scripts: Mutex<HashMap<Handle<JsScript>, ScriptData>>,
    state: Rc<Mutex<JsRuntimeState>>,
}

struct ScriptData {
    path: PathBuf,
    output: wasm_bindgen::JsValue,
}

impl FromWorld for JsRuntime {
    fn from_world(world: &mut World) -> Self {
        // Collect ops from the runtime config
        let config = world
            .remove_non_send_resource::<JsRuntimeConfig>()
            .unwrap_or_default();
        let custom_ops = config.custom_ops;
        let (ops, op_indexes, op_names) = get_ops(custom_ops);

        // Run initialization JS for each op
        for (idx, op) in ops.iter().enumerate() {
            if let Some(js) = op.js() {
                js_sys::eval(js)
                    .unwrap_or_else(|_| panic!("Error evaluating JS for op `{}`", op_names[&idx]));
            }
        }

        let state = Rc::new(Mutex::new(JsRuntimeState {
            ops,
            op_names,
            op_state: default(),
            script_info: ScriptInfo {
                path: default(),
                handle: default(),
            },
            world: default(),
        }));

        // Run our initialization script, passing in our op name map
        let op_map_json = serde_json::to_string(&op_indexes).unwrap();
        setup_js_globals(
            BevyModJsScripting {
                state: state.clone(),
            },
            &op_map_json,
        );

        Self {
            scripts: Default::default(),
            state,
        }
    }
}

impl JsRuntimeApi for JsRuntime {
    fn load_script(&self, handle: &Handle<JsScript>, script: &JsScript, _reload: bool) {
        // Set script info
        {
            let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
            state.script_info = ScriptInfo {
                path: script.path.clone(),
                handle: handle.clone_weak(),
            };
        }

        let function = js_sys::Function::new_no_args(&format!(
            r#"return ((window) => {{
                {code}
            }})(globalThis);"#,
            code = script.source
        ));

        let output = match function.call0(&JsValue::UNDEFINED) {
            Ok(output) => output,
            Err(e) => {
                error!(?script.path, "Error executing script: {:?}", e);
                return;
            }
        };

        // Clear script info
        {
            let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
            state.script_info = ScriptInfo {
                path: default(),
                handle: default(),
            };
        }

        self.scripts.try_lock().expect(LOCK_SHOULD_NOT_FAIL).insert(
            handle.clone_weak(),
            ScriptData {
                path: script.path.clone(),
                output,
            },
        );
    }

    fn has_loaded(&self, handle: &Handle<JsScript>) -> bool {
        self.scripts
            .try_lock()
            .expect(LOCK_SHOULD_NOT_FAIL)
            .contains_key(handle)
    }

    fn run_script(&self, handle: &Handle<JsScript>, fn_name_str: &str, world: &mut World) {
        {
            let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
            std::mem::swap(&mut state.world, world);
        }

        let try_run = || {
            let scripts = self.scripts.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
            let script = scripts
                .get(handle)
                .ok_or_else(|| anyhow::format_err!("Script not loaded yet"))?;
            let output = &script.output;

            {
                let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
                state.script_info = ScriptInfo {
                    path: script.path.clone(),
                    handle: handle.clone_weak(),
                };
            }

            let output: &js_sys::Object = output.dyn_ref().ok_or_else(|| {
                anyhow::format_err!("Script must have a default export that returns an object")
            })?;

            let fn_name_str = wasm_bindgen::intern(fn_name_str);
            let fn_name = wasm_bindgen::JsValue::from_str(fn_name_str);

            if let Ok(script_fn) = js_sys::Reflect::get(output, &fn_name) {
                // If a handler isn't specified for this stage, just skip this script
                if script_fn.is_undefined() {
                    return Ok(());
                }

                match script_fn.dyn_ref::<js_sys::Function>() {
                    Some(script_fn) => {
                        script_fn.call0(output).map_err(|e| {
                            anyhow::format_err!("Error running script {fn_name_str} handler: {e:?}")
                        })?;
                    }
                    None => {
                        warn!(
                            "Script exported object with {fn_name_str} field, but it was not a \
                            function. Ignoring."
                        );
                    }
                }
            }

            Ok::<_, anyhow::Error>(())
        };

        if let Err(e) = try_run() {
            // TODO: add script path to error
            error!("Error running script: {}", e);
        }

        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        state.script_info = ScriptInfo {
            path: default(),
            handle: default(),
        };
        std::mem::swap(&mut state.world, world);
    }

    fn frame_start(&self, world: &mut World) {
        let JsRuntimeState { op_state, ops, .. } =
            &mut *self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);

        for op in ops {
            op.frame_start(op_state, world);
        }
    }

    fn frame_end(&self, world: &mut World) {
        let JsRuntimeState { op_state, ops, .. } =
            &mut *self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);

        for op in ops {
            op.frame_start(op_state, world);
        }
    }
}

/// Helper trait for mapping errors to [`JsValue`]s
pub trait ToJsErr<T> {
    /// Convert the error to a [`JsValue`]
    fn to_js_error(self) -> Result<T, JsValue>;
}

impl<T, D: std::fmt::Display> ToJsErr<T> for Result<T, D> {
    fn to_js_error(self) -> Result<T, JsValue> {
        match self {
            Ok(ok) => Ok(ok),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    }
}
