use super::{
    v8_utils::{create_value_ref_object, ValueRefObject},
    WorldResource,
};
use crate::runtime::types::{JsComponentId, JsEntity};
use bevy::ecs::component::ComponentId;
use bevy_ecs_dynamic::reflect_value_ref::query::EcsValueRefQuery;
use deno_core::{error::AnyError, op, v8, OpState, ResourceId};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct QueryDescriptor {
    components: Vec<JsComponentId>,
}

#[derive(Serialize)]
pub struct JsQueryItem {
    entity: JsEntity,
    components: Vec<ValueRefObject<'static>>,
}

#[op(v8)]
pub fn op_world_query(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    rid: ResourceId,
    descriptor: QueryDescriptor,
) -> Result<Vec<JsQueryItem>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    let components: Vec<ComponentId> = descriptor
        .components
        .iter()
        .map(ComponentId::from)
        .collect();

    let mut query = EcsValueRefQuery::new(&world, &components);

    let results = query
        .iter(&world)
        .map(|item| {
            let components = item
                .items
                .into_iter()
                .map(|value| unsafe { create_value_ref_object(scope, value.into()) })
                .collect();

            JsQueryItem {
                entity: item.entity.into(),
                components,
            }
        })
        .collect();

    Ok(results)
}
