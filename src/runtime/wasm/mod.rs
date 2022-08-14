use std::path::PathBuf;
use std::rc::Rc;

use bevy::ecs::component::ComponentId;
use bevy::utils::{HashMap, HashSet};
use bevy::{
    prelude::*,
    utils::tracing::{event, span, Level},
};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_mutex::Mutex;

use super::JsRuntimeApi;
use crate::asset::JsScript;
use crate::runtime::types::{JsComponentInfo, JsEntity};

const LOCK_SHOULD_NOT_FAIL: &str =
    "Mutex lock should not fail because there should be no concurrent access";

const WORLD_RID: u32 = 0;

#[wasm_bindgen]
struct BevyModJsScripting {
    state: Rc<Mutex<JsRuntimeState>>,
}

#[derive(Default)]
struct JsRuntimeState {
    current_script_path: PathBuf,
    world: World,
}

#[wasm_bindgen]
impl BevyModJsScripting {
    pub fn op_log(&self, level: &str, text: &str) {
        let state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let path = &state.current_script_path;

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

    pub fn op_world_tostring(&self, rid: u32) -> String {
        assert_eq!(rid, WORLD_RID);
        let state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let world = &state.world;

        format!("{world:?}")
    }

    pub fn op_world_components(&self, rid: u32) -> JsValue {
        assert_eq!(rid, WORLD_RID);
        let state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let world = &state.world;

        let resource_components: HashSet<ComponentId> =
            world.archetypes().resource().components().collect();

        let infos = world
            .components()
            .iter()
            .filter(|info| !resource_components.contains(&info.id()))
            .map(JsComponentInfo::from)
            .collect::<Vec<_>>();

        serde_wasm_bindgen::to_value(&infos).unwrap()
    }

    pub fn op_world_resources(&self, rid: u32) -> JsValue {
        assert_eq!(rid, WORLD_RID);
        let state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let world = &state.world;

        let infos = world
            .archetypes()
            .resource()
            .components()
            .map(|id| world.components().get_info(id).unwrap())
            .map(JsComponentInfo::from)
            .collect::<Vec<_>>();

        serde_wasm_bindgen::to_value(&infos).unwrap()
    }

    pub fn op_world_entities(&self, rid: u32) -> JsValue {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let world = &mut state.world;

        let entities = world
            .query::<Entity>()
            .iter(world)
            .map(JsEntity::from)
            .collect::<Vec<_>>();

        serde_wasm_bindgen::to_value(&entities).unwrap()
    }
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
