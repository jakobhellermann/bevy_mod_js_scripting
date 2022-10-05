use type_map::TypeMap;

use crate::runtime::{JsRuntimeOp, OpMap};

use self::types::{JsReflectFunctions, JsValueRefs};

mod info;
mod query;
mod resource;
pub mod types;
mod value;
mod world;

pub fn insert_ecs_ops(ops: &mut OpMap) {
    ops.insert("ecs_js", Box::new(EcsJs));
    ops.insert("ecs_world_to_string", Box::new(info::ecs_world_to_string));
    ops.insert("ecs_world_components", Box::new(info::ecs_world_components));
    ops.insert("ecs_world_resources", Box::new(info::ecs_world_resources));
    ops.insert("ecs_world_entities", Box::new(info::ecs_world_entities));
    ops.insert(
        "ecs_world_query_collect",
        Box::new(query::ecs_world_query_collect),
    );
    ops.insert("ecs_world_query_get", Box::new(query::ecs_world_query_get));
    ops.insert(
        "ecs_world_get_resource",
        Box::new(resource::ecs_world_get_resource),
    );
    ops.insert("ecs_value_ref_get", Box::new(value::ecs_value_ref_get));
    ops.insert("ecs_value_ref_set", Box::new(value::ecs_value_ref_set));
    ops.insert("ecs_value_ref_keys", Box::new(value::ecs_value_ref_keys));
    ops.insert(
        "ecs_value_ref_to_string",
        Box::new(value::ecs_value_ref_to_string),
    );
    ops.insert("ecs_value_ref_call", Box::new(value::ecs_value_ref_call));
    ops.insert("ecs_value_ref_eq", Box::new(value::ecs_value_ref_eq));
    ops.insert("ecs_value_ref_free", Box::new(value::ecs_value_ref_free));
    ops.insert(
        "ecs_value_ref_default",
        Box::new(value::ecs_value_ref_default),
    );
    ops.insert("ecs_value_ref_patch", Box::new(value::ecs_value_ref_patch));
    ops.insert("ecs_entity_spawn", Box::new(world::ecs_entity_spawn));
    ops.insert(
        "ecs_component_insert",
        Box::new(world::ecs_component_insert),
    );
}

/// Op used to provide the JS classes and globals used to interact with the other ECS ops
struct EcsJs;
impl JsRuntimeOp for EcsJs {
    fn js(&self) -> Option<&'static str> {
        Some(include_str!("./ecs/ecs.js"))
    }
}

/// Extension trait for [`World`] that removes the [`JsValueRefs`] and [`JsReflectFunctions`]
/// resources, gives them to the closure, and adds them back to the world when the closure finishes.
///
/// Essentially a custom-made [`World::resource_scope`].
trait WithValueRefs {
    fn with_refs_and_funcs<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut JsValueRefs, &mut JsReflectFunctions) -> R;
}

impl WithValueRefs for TypeMap {
    fn with_refs_and_funcs<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut JsValueRefs, &mut JsReflectFunctions) -> R,
    {
        let mut value_refs = self.remove::<JsValueRefs>().unwrap_or_default();
        let mut reflect_functions = self.remove::<JsReflectFunctions>().unwrap_or_default();

        let r = f(self, &mut value_refs, &mut reflect_functions);

        self.insert(value_refs);
        self.insert(reflect_functions);

        r
    }
}
