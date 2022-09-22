use anyhow::{format_err, Context};
use bevy::{
    ecs::component::ComponentId,
    prelude::{default, Entity},
};
use bevy_ecs_dynamic::reflect_value_ref::{query::EcsValueRefQuery, ReflectValueRef};

use crate::runtime::OpContext;

use super::types::{ComponentIdOrBevyType, JsQueryItem, JsValueRef, JsValueRefs};

pub type QueryDescriptor = Vec<ComponentIdOrBevyType>;

pub fn ecs_world_query(
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
        .map(|ty| ty.component_id(world))
        .collect::<Result<_, _>>()?;

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
                entity: JsValueRef::new_free(Box::new(item.entity), value_refs),
                components,
            }
        })
        .collect::<Vec<_>>();

    Ok(serde_json::to_value(results)?)
}

pub fn ecs_world_get(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let (entity_value_ref, component): (JsValueRef, ComponentIdOrBevyType) =
        serde_json::from_value(args).context("component query")?;
    let entity: Entity = {
        let value_ref: &ReflectValueRef = value_refs
            .get(entity_value_ref.key)
            .ok_or_else(|| format_err!("Value ref doesn't exist"))?;

        let borrow = value_ref.get(world)?;

        *borrow
            .downcast_ref()
            .ok_or_else(|| format_err!("Value passed not an entity"))?
    };

    let component = component.component_id(world)?;

    let mut query = EcsValueRefQuery::new(world, &[component]);
    let results = query
        .iter(world)
        .filter(|x| x.entity == entity)
        .map(|item| {
            item.items
                .into_iter()
                .map(|value| JsValueRef {
                    key: value_refs.insert(ReflectValueRef::ecs_ref(value)),
                    function: None,
                })
                .next()
        })
        .next()
        .flatten();

    Ok(serde_json::to_value(results)?)
}
