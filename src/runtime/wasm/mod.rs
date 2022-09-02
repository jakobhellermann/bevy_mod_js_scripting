mod ecs;
mod log;

use std::path::PathBuf;
use std::rc::Rc;

use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_dynamic::reflect_value_ref::ReflectValueRef;
use bevy_reflect_fns::ReflectFunction;
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_mutex::{Mutex, MutexRef};

use super::JsRuntimeApi;
use crate::asset::JsScript;
use crate::runtime::types::JsEntity;

/// Panic message when a mutex lock fails
const LOCK_SHOULD_NOT_FAIL: &str =
    "Mutex lock should not fail because there should be no concurrent access";

/// Error message thrown when a value ref refers to a value that doesn't exist.
const REF_NOT_EXIST: &str =
    "Value referenced does not exist. Each value ref is only valid for the duration of the script \
    execution that it was created in. You may have attempted to use a value from a previous sciprt \
    run.";

const WORLD_RID: u32 = 0;

slotmap::new_key_type! {
    struct JsValueRefKey;
    struct ReflectFunctionKey;
}

#[derive(Serialize, Deserialize, Debug)]
struct JsValueRef {
    key: JsValueRefKey,
    function: Option<ReflectFunctionKey>,
}

#[derive(Serialize)]
struct JsQueryItem {
    entity: JsEntity,
    components: Vec<JsValueRef>,
}

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

#[derive(Default)]
struct JsRuntimeState {
    current_script_path: PathBuf,
    world: World,
    value_refs: SlotMap<JsValueRefKey, ReflectValueRef>,
    reflect_functions: SlotMap<ReflectFunctionKey, ReflectFunction>,
}

#[wasm_bindgen(module = "/src/runtime/wasm/wasm_setup.js")]
extern "C" {
    fn setup_js_globals(bevy_mod_js_scripting: BevyModJsScripting);
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
    fn from_world(_: &mut World) -> Self {
        let state = Rc::new(Mutex::new(JsRuntimeState::default()));

        setup_js_globals(BevyModJsScripting {
            state: state.clone(),
        });

        js_sys::eval(include_str!("../js/ecs.js")).expect("Eval Init JS");
        js_sys::eval(include_str!("../js/log.js")).expect("Eval Init JS");

        Self {
            scripts: Default::default(),
            state,
        }
    }
}

impl JsRuntimeApi for JsRuntime {
    fn load_script(&self, handle: &Handle<JsScript>, script: &JsScript, _reload: bool) {
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

    fn run_script(&self, handle: &Handle<JsScript>, stage: &CoreStage, world: &mut World) {
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
                state.value_refs.clear();
                state.reflect_functions.clear();
                state.current_script_path = script.path.clone();
            }

            let output: &js_sys::Object = output.dyn_ref().ok_or_else(|| {
                anyhow::format_err!("Script must have a default export that returns an object")
            })?;

            let fn_name = match stage {
                CoreStage::First => "first",
                CoreStage::PreUpdate => "pre_update",
                CoreStage::Update => "update",
                CoreStage::PostUpdate => "post_update",
                CoreStage::Last => "last",
            };
            let fn_name_str = wasm_bindgen::intern(fn_name);
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
        state.current_script_path = PathBuf::new();
        std::mem::swap(&mut state.world, world);
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

/// Helper trait to get a reflect value ref from the `value_refs` slotmap on [`JsRuntimeState`].
trait GetReflectValueRef {
    /// Casts a [`JsValue`] to a [`JsValueRef`] and loads its [`ReflectValueRef`].
    fn get_reflect_value_ref(&self, value: JsValue) -> Result<&ReflectValueRef, JsValue>;
}

impl GetReflectValueRef for SlotMap<JsValueRefKey, ReflectValueRef> {
    fn get_reflect_value_ref(&self, js_value: JsValue) -> Result<&ReflectValueRef, JsValue> {
        let value_ref: JsValueRef = serde_wasm_bindgen::from_value(js_value)?;

        self.get(value_ref.key).ok_or(REF_NOT_EXIST).to_js_error()
    }
}
