use anyhow::Context;
use bevy::prelude::default;
use bevy_ecs_dynamic::reflect_value_ref::{EcsValueRef, ReflectValueRef};

use crate::runtime::OpContext;

use super::types::{ComponentIdOrBevyType, JsValueRef, JsValueRefs};

pub fn ecs_world_get_resource(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let (component_id,): (ComponentIdOrBevyType,) =
        serde_json::from_value(args).context("parse args")?;
    let component_id = component_id.component_id(world)?;

    let value_ref = EcsValueRef::resource(world, component_id)?;

    let value_ref = JsValueRef {
        key: value_refs.insert(ReflectValueRef::ecs_ref(value_ref)),
        function: None,
    };

    Ok(serde_json::to_value(value_ref)?)
}
