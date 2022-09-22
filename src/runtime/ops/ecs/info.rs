use bevy::{
    ecs::component::ComponentId,
    prelude::{default, Entity},
    utils::HashSet,
};

use crate::{runtime::OpContext, JsValueRef, JsValueRefs};

use super::types::JsComponentInfo;

pub fn ecs_world_to_string(
    _context: OpContext,
    world: &mut bevy::prelude::World,
    _args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::Value::String(format!("{world:?}")))
}

pub fn ecs_world_components(
    _context: OpContext,
    world: &mut bevy::prelude::World,
    _args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let resource_components: HashSet<ComponentId> =
        world.archetypes().resource().components().collect();

    let infos = world
        .components()
        .iter()
        .filter(|info| !resource_components.contains(&info.id()))
        .map(JsComponentInfo::from)
        .collect::<Vec<_>>();

    Ok(serde_json::to_value(&infos)?)
}

pub fn ecs_world_resources(
    _context: OpContext,
    world: &mut bevy::prelude::World,
    _args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let infos = world
        .archetypes()
        .resource()
        .components()
        .map(|id| world.components().get_info(id).unwrap())
        .map(JsComponentInfo::from)
        .collect::<Vec<_>>();

    Ok(serde_json::to_value(infos)?)
}

pub fn ecs_world_entities(
    context: OpContext,
    world: &mut bevy::prelude::World,
    _args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let entities = world
        .query::<Entity>()
        .iter(world)
        .map(|e| JsValueRef::new_free(Box::new(e), value_refs))
        .collect::<Vec<_>>();

    Ok(serde_json::to_value(entities)?)
}
