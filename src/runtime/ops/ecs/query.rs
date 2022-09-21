use anyhow::Context;
use bevy::{ecs::component::ComponentId, prelude::default};
use bevy_ecs_dynamic::reflect_value_ref::{query::EcsValueRefQuery, ReflectValueRef};

use crate::runtime::OpContext;

use super::types::{JsQueryItem, JsValueRef, JsValueRefs, QueryDescriptor};

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

    Ok(serde_json::to_value(results)?)
}
