use std::path::PathBuf;
use std::rc::Rc;

use bevy::utils::HashMap;
use bevy::{
    prelude::*,
    utils::tracing::{event, span, Level},
};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_mutex::Mutex;

use crate::asset::JsScript;

use super::JsRuntimeApi;

const LOCK_SHOULD_NOT_FAIL: &str =
    "Mutex lock should not fail because there should be no concurrent access";

#[wasm_bindgen]
struct BevyModJsScripting {
    state: Rc<Mutex<JsRuntimeState>>,
}

#[derive(Default)]
struct JsRuntimeState {
    current_script_path: PathBuf,
}

#[wasm_bindgen]
impl BevyModJsScripting {
    pub fn op_log(&self, level: &str, text: &str) {
        let data = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let path = &data.current_script_path;

        let level: Level = level.parse().unwrap_or(Level::INFO);
        if level == Level::TRACE {
            let _span = span!(Level::TRACE, "script", ?path).entered();
            event!(target: "js_runtime", Level::TRACE, "{text}");
        } else if level == Level::DEBUG {
            let _span = span!(Level::DEBUG, "script", ?path).entered();
            event!(target: "js_runtime", Level::DEBUG, "{text}");
        } else if level == Level::INFO {
            let _span = span!(Level::INFO, "script", ?path).entered();
            event!(target: "js_runtime", Level::INFO, "{text}");
        } else if level == Level::WARN {
            let _span = span!(Level::WARN, "script", ?path).entered();
            event!(target: "js_runtime", Level::WARN, "{text}");
        } else if level == Level::ERROR {
            let _span = span!(Level::ERROR, "script", ?path).entered();
            event!(target: "js_runtime", Level::ERROR, "{text}");
        }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "Object")]
    type BevyScriptJsObject;

    #[wasm_bindgen(method, catch)]
    fn first(this: &BevyScriptJsObject) -> Result<(), JsValue>;
    #[wasm_bindgen(method, catch)]
    fn pre_update(this: &BevyScriptJsObject) -> Result<(), JsValue>;
    #[wasm_bindgen(method, catch)]
    fn update(this: &BevyScriptJsObject) -> Result<(), JsValue>;
    #[wasm_bindgen(method, catch)]
    fn post_update(this: &BevyScriptJsObject) -> Result<(), JsValue>;
    #[wasm_bindgen(method, catch)]
    fn last(this: &BevyScriptJsObject) -> Result<(), JsValue>;
}

#[wasm_bindgen(module = "/src/runtime/wasm/wasm_setup.js")]
extern "C" {
    fn setup_js_globals(bevy_mod_js_scripting: BevyModJsScripting);
}

impl BevyScriptJsObject {
    fn has_fn(&self, name: &str) -> bool {
        let name = wasm_bindgen::intern(name);

        self.dyn_ref()
            .map(|obj: &js_sys::Object| obj.has_own_property(&JsValue::from_str(name)))
            .unwrap_or_default()
    }
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

    fn run_script(&self, handle: &Handle<JsScript>, stage: &CoreStage, _world: &mut World) {
        let try_run = || {
            let scripts = self.scripts.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
            let script = scripts
                .get(handle)
                .ok_or_else(|| anyhow::format_err!("Script not loaded yet"))?;
            let output = &script.output;

            let output: &BevyScriptJsObject = output.dyn_ref().ok_or_else(|| {
                anyhow::format_err!(
                    "Script must export an object with an object with an `update` function."
                )
            })?;

            {
                let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
                state.current_script_path = script.path.clone();
            }

            match stage {
                CoreStage::First => {
                    if output.has_fn("first") {
                        output.first()
                    } else {
                        Ok(())
                    }
                }
                CoreStage::PreUpdate => {
                    if output.has_fn("pre_update") {
                        output.pre_update()
                    } else {
                        Ok(())
                    }
                }
                CoreStage::Update => {
                    if output.has_fn("update") {
                        output.update()
                    } else {
                        Ok(())
                    }
                }
                CoreStage::PostUpdate => {
                    if output.has_fn("post_update") {
                        output.post_update()
                    } else {
                        Ok(())
                    }
                }
                CoreStage::Last => {
                    if output.has_fn("last") {
                        output.last()
                    } else {
                        Ok(())
                    }
                }
            }
            .map_err(|e| anyhow::format_err!("Error executing script function: {e:?}"))?;

            Ok::<_, anyhow::Error>(())
        };

        if let Err(e) = try_run() {
            // TODO: add script path to error
            error!("Error running script: {}", e);
        }
    }
}
