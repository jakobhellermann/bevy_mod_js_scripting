use bevy::{ecs::component::ComponentId, prelude::Entity, utils::HashSet};
use type_map::TypeMap;

use super::types::{JsComponentInfo, JsEntity};

pub fn ecs_world_to_string(
    _op_state: &mut TypeMap,
    _script_info: &crate::runtime::ScriptInfo,
    world: &mut bevy::prelude::World,
    _args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::Value::String(format!("{world:?}")))
}

pub fn ecs_world_components(
    _op_state: &mut TypeMap,
    _script_info: &crate::runtime::ScriptInfo,
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
    _op_state: &mut TypeMap,
    _script_info: &crate::runtime::ScriptInfo,
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
    _op_state: &mut TypeMap,
    _script_info: &crate::runtime::ScriptInfo,
    world: &mut bevy::prelude::World,
    _args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let entities = world
        .query::<Entity>()
        .iter(world)
        .map(JsEntity::from)
        .collect::<Vec<_>>();

    Ok(serde_json::to_value(entities)?)
}
