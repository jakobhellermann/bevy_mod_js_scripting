use anyhow::Context;
use bevy::{
    ecs::component::ComponentId,
    prelude::{default, Entity},
};
use bevy_ecs_dynamic::reflect_value_ref::query::EcsValueRefQuery;

use crate::runtime::OpContext;

use super::types::{ComponentIdOrBevyType, JsQueryItem, JsValueRef, JsValueRefs};

pub type QueryDescriptor = Vec<ComponentIdOrBevyType>;

/// Queries world and collects results into a JS array
pub fn ecs_world_query_collect(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let (descriptor,): (QueryDescriptor,) =
        serde_json::from_value(args).context("Parse world query descriptor")?;

    let components: Vec<ComponentId> = descriptor
        .iter()
        .map(|ty| ty.component_id(world, context.type_registry))
        .collect::<Result<_, _>>()?;

    let mut query = EcsValueRefQuery::new(world, &components);
    let results = query
        .iter(world)
        .map(|item| {
            let components = item
                .items
                .into_iter()
                .map(|value| JsValueRef::new_ecs(value, value_refs))
                .collect();

            JsQueryItem {
                entity: JsValueRef::new_free(Box::new(item.entity), value_refs),
                components,
            }
        })
        .collect::<Vec<_>>();

    Ok(serde_json::to_value(results)?)
}

/// Queries world and gets the components of a specific entity
pub fn ecs_world_query_get(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let (entity_value_ref, descriptor): (JsValueRef, QueryDescriptor) =
        serde_json::from_value(args).context("component query")?;
    let entity: Entity = entity_value_ref.get_entity(world, value_refs)?;

    let components: Vec<ComponentId> = descriptor
        .iter()
        .map(|ty| ty.component_id(world, context.type_registry))
        .collect::<Result<_, _>>()?;

    let mut query = EcsValueRefQuery::new(world, &components);
    let result = query
        .get(world, entity)
        .map(|components| {
            components
                .into_iter()
                .map(|value| JsValueRef::new_ecs(value, value_refs))
                .collect::<Vec<_>>()
        })
        .ok();

    Ok(serde_json::to_value(result)?)
}
