use bevy::{
    ecs::component::{ComponentId, ComponentInfo},
    prelude::*,
    utils::HashSet,
};
use deno_core::{
    error::AnyError, include_js_files, op, serde::Serialize, Extension, OpState, ResourceId,
};

use super::WorldResource;

#[op]
fn op_world_tostring(state: &mut OpState, rid: ResourceId) -> Result<String, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    Ok(format!("{world:?}"))
}

#[derive(Serialize)]
struct JsComponentId {
    index: usize,
}
impl From<ComponentId> for JsComponentId {
    fn from(id: ComponentId) -> Self {
        JsComponentId { index: id.index() }
    }
}

#[derive(Serialize)]
struct JsComponentInfo {
    id: JsComponentId,
    name: String,
    size: usize,
}
impl From<&ComponentInfo> for JsComponentInfo {
    fn from(info: &ComponentInfo) -> Self {
        JsComponentInfo {
            id: info.id().into(),
            name: info.name().to_string(),
            size: info.layout().size(),
        }
    }
}

#[derive(Serialize)]
struct JsEntity {
    bits: u64,
    id: u32,
    generation: u32,
}
impl From<Entity> for JsEntity {
    fn from(entity: Entity) -> Self {
        JsEntity {
            bits: entity.to_bits(),
            id: entity.id(),
            generation: entity.generation(),
        }
    }
}

#[op]
fn op_world_components(
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
fn op_world_resources(
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
fn op_world_entities(state: &mut OpState, rid: ResourceId) -> Result<Vec<JsEntity>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let mut world = world.world.borrow_mut();

    let entities = world
        .query::<Entity>()
        .iter(&world)
        .map(JsEntity::from)
        .collect();

    Ok(entities)
}

pub fn extension() -> Extension {
    Extension::builder()
        .ops(vec![
            op_world_tostring::decl(),
            op_world_components::decl(),
            op_world_resources::decl(),
            op_world_entities::decl(),
        ])
        .js(include_js_files!(prefix "bevy", "js/ecs.js",))
        .build()
}
