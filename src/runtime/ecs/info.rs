use super::{
    types::{JsComponentInfo, JsEntity},
    WorldResource,
};
use bevy::{ecs::component::ComponentId, prelude::*, utils::HashSet};
use deno_core::{error::AnyError, op, OpState, ResourceId};

#[op]
pub fn op_world_tostring(state: &mut OpState, rid: ResourceId) -> Result<String, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    Ok(format!("{world:?}"))
}

#[op]
pub fn op_world_components(
    state: &mut OpState,
    rid: ResourceId,
) -> Result<Vec<JsComponentInfo>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    let resource_components: HashSet<ComponentId> =
        world.archetypes().resource().components().collect();

    let infos = world
        .components()
        .iter()
        .filter(|info| !resource_components.contains(&info.id()))
        .map(JsComponentInfo::from)
        .collect();

    Ok(infos)
}

#[op]
pub fn op_world_resources(
    state: &mut OpState,
    rid: ResourceId,
) -> Result<Vec<JsComponentInfo>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    let infos = world
        .archetypes()
        .resource()
        .components()
        .map(|id| world.components().get_info(id).unwrap())
        .map(JsComponentInfo::from)
        .collect();

    Ok(infos)
}

#[op]
pub fn op_world_entities(state: &mut OpState, rid: ResourceId) -> Result<Vec<JsEntity>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let mut world = world.world.borrow_mut();

    let entities = world
        .query::<Entity>()
        .iter(&world)
        .map(JsEntity::from)
        .collect();

    Ok(entities)
}
