use std::sync::Arc;

use bevy::prelude::*;

use crate::{
    runtime::{JsRuntimeOp, OpMap},
    JsReflectFunctions, JsValueRefs,
};

mod info;
mod query;
mod resource;
mod value;

pub fn insert_ecs_ops(ops: &mut OpMap) {
    ops.insert("ecs_js", Arc::new(EcsJs));
    ops.insert("ecs_world_to_string", Arc::new(info::ecs_world_to_string));
    ops.insert("ecs_world_components", Arc::new(info::ecs_world_components));
    ops.insert("ecs_world_resources", Arc::new(info::ecs_world_resources));
    ops.insert("ecs_world_entities", Arc::new(info::ecs_world_entities));
    ops.insert("ecs_world_query", Arc::new(query::ecs_world_query));
    ops.insert(
        "ecs_world_get_resource",
        Arc::new(resource::ecs_world_get_resource),
    );
    ops.insert("ecs_value_ref_get", Arc::new(value::ecs_value_ref_get));
    ops.insert("ecs_value_ref_set", Arc::new(value::ecs_value_ref_set));
    ops.insert("ecs_value_ref_keys", Arc::new(value::ecs_value_ref_keys));
    ops.insert(
        "ecs_value_ref_to_string",
        Arc::new(value::ecs_value_ref_to_string),
    );
    ops.insert("ecs_value_ref_call", Arc::new(value::ecs_value_ref_call));
    ops.insert("ecs_value_ref_free", Arc::new(value::ecs_value_ref_free));
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
    fn with_value_refs<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut JsValueRefs, &mut JsReflectFunctions) -> R;
}

impl WithValueRefs for World {
    fn with_value_refs<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self, &mut JsValueRefs, &mut JsReflectFunctions) -> R,
    {
        let mut value_refs = self.remove_non_send_resource::<JsValueRefs>().unwrap();
        let mut reflect_functions = self.remove_resource::<JsReflectFunctions>().unwrap();

        let r = f(self, &mut value_refs, &mut reflect_functions);

        self.insert_non_send_resource(value_refs);
        self.insert_resource(reflect_functions);

        r
    }
}
