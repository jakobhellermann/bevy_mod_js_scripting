use bevy::prelude::*;
use bevy::utils::HashMap;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_mutex::Mutex;

use crate::asset::JsScript;

use super::JsRuntimeApi;

const LOCK_SHOULD_NOT_FAIL: &str =
    "Mutex lock should not fail because there should be no concurrent access";

// #[wasm_bindgen]
// struct Punchy;

// #[wasm_bindgen]
// impl Punchy {
//     pub fn log(&self, message: &str, level: &str) {
//         let script = current_script_path();
//         match level {
//             "error" => error!(script, "{}", message),
//             "warn" => warn!(script, "{}", message),
//             "debug" => debug!(script, "{}", message),
//             "trace" => trace!(script, "{}", message),
//             // Default to info
//             _ => info!(script, "{}", message),
//         };
//     }
// }

#[wasm_bindgen]
extern "C" {

    #[wasm_bindgen(js_name = "Object")]
    type ScriptObject;

    #[wasm_bindgen(method, catch)]
    fn update(this: &ScriptObject) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    fn update(this: &ScriptObject) -> Result<(), JsValue>;
}

pub struct JsRuntime {
    scripts: Mutex<HashMap<Handle<JsScript>, wasm_bindgen::JsValue>>,
}

impl FromWorld for JsRuntime {
    fn from_world(_: &mut World) -> Self {
        js_sys::eval(include_str!("./wasm_setup.js")).expect("Eval Init JS");
        js_sys::eval(include_str!("../js/ecs.js")).expect("Eval Init JS");
        js_sys::eval(include_str!("../js/log.js")).expect("Eval Init JS");

        Self {
            scripts: Default::default(),
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

        self.scripts
            .try_lock()
            .expect(LOCK_SHOULD_NOT_FAIL)
            .insert(handle.clone_weak(), output);
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
            let output = scripts
                .get(handle)
                .ok_or_else(|| anyhow::format_err!("Script not loaded yet"))?;

            let output: &ScriptObject = output.dyn_ref().ok_or_else(|| {
                anyhow::format_err!(
                    "Script must export an object with an object with an `update` function."
                )
            })?;

            match stage {
                CoreStage::Update => output.update(),
                _ => return Ok(()),
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
