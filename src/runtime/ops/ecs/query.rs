use anyhow::Context;
use bevy::ecs::component::ComponentId;
use bevy_ecs_dynamic::reflect_value_ref::{query::EcsValueRefQuery, ReflectValueRef};

use crate::runtime::types::{JsQueryItem, JsValueRef, QueryDescriptor};

use super::WithValueRefs;

pub fn ecs_world_query(
    _script_info: &crate::runtime::ScriptInfo,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    world.with_value_refs(|world, value_refs, _| {
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
    })
}
