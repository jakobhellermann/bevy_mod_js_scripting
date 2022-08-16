use std::any::TypeId;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use bevy::ecs::component::ComponentId;
use bevy::utils::{HashMap, HashSet};
use bevy::{
    prelude::*,
    utils::tracing::{event, span, Level},
};
use bevy_ecs_dynamic::reflect_value_ref::query::EcsValueRefQuery;
use bevy_ecs_dynamic::reflect_value_ref::{EcsValueRef, ReflectValueRef};
use bevy_reflect::{ReflectRef, TypeRegistryArc};
use bevy_reflect_fns::{PassMode, ReflectArg, ReflectFunction};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_mutex::Mutex;

use super::JsRuntimeApi;
use crate::asset::JsScript;
use crate::runtime::types::{
    ComponentIdOrBevyType, JsComponentInfo, JsEntity, Primitive, QueryDescriptor,
    ReflectArgIntermediate, ReflectArgIntermediateValue,
};

const LOCK_SHOULD_NOT_FAIL: &str =
    "Mutex lock should not fail because there should be no concurrent access";

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

#[derive(Default)]
struct JsRuntimeState {
    current_script_path: PathBuf,
    world: World,
    value_refs: SlotMap<JsValueRefKey, ReflectValueRef>,
    reflect_functions: SlotMap<ReflectFunctionKey, ReflectFunction>,
}

macro_rules! try_downcast_leaf_get {
    ($value:ident for $($ty:ty $(,)?),*) => {
        $(if let Some(value) = $value.downcast_ref::<$ty>() {
            let value = serde_wasm_bindgen::to_value(value)?;
            return Ok(value);
        })*
    };
}

macro_rules! try_downcast_leaf_set {
    ($value:ident <- $new_value:ident for $($ty:ty $(,)?),*) => {
        $(if let Some(value) = $value.downcast_mut::<$ty>() {
            *value = serde_wasm_bindgen::from_value($new_value)?;
            return Ok(());
        })*
    };
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

    // TODO: Get rid of all the unwraps and throw proper errors
    pub fn op_world_query(&self, rid: u32, query: JsValue) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let JsRuntimeState {
            world, value_refs, ..
        } = &mut *state;

        let descriptor: QueryDescriptor = serde_wasm_bindgen::from_value(query)?;

        let components: Vec<ComponentId> = descriptor
            .components
            .iter()
            .map(ComponentId::from)
            .collect();

        let mut query = EcsValueRefQuery::new(world, &components);
        let results = query
            .iter(world)
            .map(|item| {
                let components = item
                    .items
                    .into_iter()
                    .map(|value| JsValueRef {
                        key: value_refs.insert(ReflectValueRef::ecs_ref(value)),
                        function: None,
                    })
                    .collect();

                JsQueryItem {
                    entity: item.entity.into(),
                    components,
                }
            })
            .collect::<Vec<_>>();

        Ok(serde_wasm_bindgen::to_value(&results).unwrap())
    }

    pub fn op_world_get_resource(
        &self,
        rid: u32,
        component_id: JsValue,
    ) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let JsRuntimeState {
            world, value_refs, ..
        } = &mut *state;

        let component_id: ComponentIdOrBevyType = serde_wasm_bindgen::from_value(component_id)?;
        let component_id = component_id.component_id(world).unwrap();

        let value_ref = EcsValueRef::resource(world, component_id);
        if world.get_resource_by_id(component_id).is_none() || value_ref.is_err() {
            return Ok(JsValue::NULL);
        }

        let value_ref = value_ref.unwrap();
        let value_ref = JsValueRef {
            key: value_refs.insert(ReflectValueRef::ecs_ref(value_ref)),
            function: None,
        };

        Ok(serde_wasm_bindgen::to_value(&value_ref)?)
    }

    pub fn op_value_ref_get(
        &self,
        rid: u32,
        value_ref: JsValue,
        path: &str,
    ) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let JsRuntimeState {
            world,
            value_refs,
            reflect_functions,
            ..
        } = &mut *state;

        let value_ref: JsValueRef = serde_wasm_bindgen::from_value(value_ref)?;
        let value_ref = match value_refs.get(value_ref.key) {
            Some(value_ref) => value_ref,
            None => return Ok(JsValue::NULL),
        };

        let type_registry = world.resource::<TypeRegistryArc>();
        let type_registry = type_registry.read();

        let reflect_methods = type_registry.get_type_data::<bevy_reflect_fns::ReflectMethods>(
            value_ref.get(world).unwrap().type_id(),
        );

        if let Some(reflect_methods) = reflect_methods {
            let method_name = path.trim_start_matches('.');
            if let Some(reflect_function) = reflect_methods.get(method_name.trim_start_matches('.'))
            {
                let value = JsValueRef {
                    key: value_refs.insert(value_ref.clone()),
                    function: Some(reflect_functions.insert(reflect_function.clone())),
                };

                return Ok(serde_wasm_bindgen::to_value(&value)?);
            }
        }
        let value_ref = value_ref.append_path(path, world).unwrap();

        {
            let value = value_ref.get(world).unwrap();

            try_downcast_leaf_get!(value for
                u8, u16, u32, u64, u128, usize,
                i8, i16, i32, i64, i128, isize,
                String, char, bool, f32, f64
            );
        }

        let object = JsValueRef {
            key: value_refs.insert(value_ref),
            function: None,
        };

        Ok(serde_wasm_bindgen::to_value(&object)?)
    }

    pub fn op_value_ref_set(
        &self,
        rid: u32,
        value_ref: JsValue,
        path: &str,
        new_value: JsValue,
    ) -> Result<(), JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let JsRuntimeState {
            world, value_refs, ..
        } = &mut *state;

        let value_ref: JsValueRef = serde_wasm_bindgen::from_value(value_ref)?;
        let value_ref = match value_refs.get_mut(value_ref.key) {
            Some(value_ref) => value_ref,
            None => return Ok(()),
        };
        let mut value_ref = value_ref.append_path(path, world).unwrap();

        let mut reflect = value_ref.get_mut(world).unwrap();

        try_downcast_leaf_set!(reflect <- new_value for
            u8, u16, u32, u64, u128, usize,
            i8, i16, i32, i64, i128, isize,
            String, char, bool, f32, f64
        );

        Err(JsValue::from_str(&format!(
            "could not set value reference: type `{}` is not a primitive type",
            reflect.type_name(),
        )))
    }

    pub fn op_value_ref_keys(&self, rid: u32, value_ref: JsValue) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let JsRuntimeState {
            world, value_refs, ..
        } = &mut *state;

        let value_ref: JsValueRef = serde_wasm_bindgen::from_value(value_ref)?;
        let value_ref = match value_refs.get_mut(value_ref.key) {
            Some(value_ref) => value_ref,
            None => return Ok(JsValue::NULL),
        };
        let reflect = value_ref.get(world).unwrap();

        let fields = match reflect.reflect_ref() {
            ReflectRef::Struct(s) => (0..s.field_len())
                .map(|i| {
                    let name = s.name_at(i).ok_or_else(|| {
                        JsValue::from_str(&format!(
                            "misbehaving Reflect impl on `{}`",
                            s.type_name()
                        ))
                    })?;
                    Ok(name.to_owned())
                })
                .collect::<Result<_, JsValue>>()?,
            ReflectRef::Tuple(tuple) => (0..tuple.field_len()).map(|i| i.to_string()).collect(),
            ReflectRef::TupleStruct(tuple_struct) => (0..tuple_struct.field_len())
                .map(|i| i.to_string())
                .collect(),
            _ => Vec::new(),
        };

        Ok(serde_wasm_bindgen::to_value(&fields)?)
    }

    pub fn op_value_ref_to_string(&self, rid: u32, value_ref: JsValue) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let JsRuntimeState {
            world, value_refs, ..
        } = &mut *state;

        let value_ref: JsValueRef = serde_wasm_bindgen::from_value(value_ref)?;
        let value_ref = match value_refs.get(value_ref.key) {
            Some(value_ref) => value_ref,
            None => return Ok(JsValue::NULL),
        };

        let reflect = value_ref.get(world).unwrap();

        Ok(JsValue::from_str(&format!("{reflect:?}")))
    }

    pub fn op_value_ref_call(
        &self,
        rid: u32,
        receiver: JsValue,
        args: JsValue,
    ) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let JsRuntimeState {
            world,
            value_refs,
            reflect_functions,
            ..
        } = &mut *state;

        #[derive(Deserialize, Debug)]
        #[serde(untagged)]
        enum ArgType {
            JsValueRef(JsValueRef),
            Number(f64),
        }

        // Deserialize the receiver
        let receiver: JsValueRef = serde_wasm_bindgen::from_value(receiver)?;

        // Get the receivers reflect_function
        let method = match receiver.function {
            Some(function) => match reflect_functions.get_mut(function) {
                Some(function) => function,
                None => panic!(),
            },
            None => panic!(),
        };
        let receiver = match value_refs.get(receiver.key) {
            Some(value_ref) => value_ref.clone(),
            None => panic!(),
        };

        let args: js_sys::Array = args.dyn_into().unwrap();

        let receiver_pass_mode = method.signature[0].0;
        let receiver_intermediate = match receiver_pass_mode {
            PassMode::Ref => ReflectArgIntermediateValue::Ref(receiver.get(world).unwrap()),
            PassMode::RefMut => {
                unimplemented!("values passed by mutable reference in reflect fn call")
            }
            PassMode::Owned => ReflectArgIntermediateValue::Owned(receiver.get(world).unwrap()),
        };
        let mut receiver_intermediate = ReflectArgIntermediate::Value(receiver_intermediate);

        let mut arg_intermediates = args
            .iter()
            .zip(method.signature.iter().skip(1))
            .map(|(arg, &(pass_mode, type_id))| {
                let downcast_primitive = match type_id {
                    type_id if type_id == TypeId::of::<f32>() => {
                        Some(Primitive::f32(serde_wasm_bindgen::from_value(arg.clone())?))
                    }
                    type_id if type_id == TypeId::of::<f64>() => {
                        Some(Primitive::f64(serde_wasm_bindgen::from_value(arg.clone())?))
                    }
                    type_id if type_id == TypeId::of::<i32>() => {
                        Some(Primitive::i32(serde_wasm_bindgen::from_value(arg.clone())?))
                    }
                    type_id if type_id == TypeId::of::<u32>() => {
                        Some(Primitive::u32(serde_wasm_bindgen::from_value(arg.clone())?))
                    }
                    _ => None,
                };
                if let Some(primitive) = downcast_primitive {
                    return Ok(ReflectArgIntermediate::Primitive(primitive, pass_mode));
                }

                let value: JsValueRef = serde_wasm_bindgen::from_value(arg)?;
                let value = match value_refs.get(value.key) {
                    Some(value_ref) => value_ref,
                    None => panic!(),
                };
                let value = match pass_mode {
                    PassMode::Ref => ReflectArgIntermediateValue::Ref(value.get(world).unwrap()),
                    PassMode::RefMut => {
                        unimplemented!("values passed by mutable reference in reflect fn call")
                    }
                    PassMode::Owned => {
                        ReflectArgIntermediateValue::Owned(value.get(world).unwrap())
                    }
                };

                Ok(ReflectArgIntermediate::Value(value))
            })
            .collect::<Result<Vec<_>, JsValue>>()?;
        let mut args: Vec<ReflectArg> = std::iter::once(&mut receiver_intermediate)
            .chain(arg_intermediates.iter_mut())
            .map(|intermediate| intermediate.as_arg())
            .collect();

        let ret = method.call(args.as_mut_slice()).unwrap();
        let ret = Rc::new(RefCell::new(ret));
        let ret = ReflectValueRef::free(ret);

        drop(args);
        drop(arg_intermediates);
        drop(receiver_intermediate);

        let ret = JsValueRef {
            key: value_refs.insert(ret),
            function: None,
        };

        Ok(serde_wasm_bindgen::to_value(&ret)?)
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
