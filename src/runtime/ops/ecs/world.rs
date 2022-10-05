use anyhow::{format_err, Context};
use bevy::prelude::{default, ReflectComponent};
use bevy_reflect::TypeRegistryArc;

use crate::{JsValueRef, JsValueRefs, OpContext};

pub fn ecs_entity_spawn(
    context: OpContext,
    world: &mut bevy::prelude::World,
    _args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let entity = world.spawn().id();
    let value_ref = JsValueRef::new_free(Box::new(entity), value_refs);

    Ok(serde_json::to_value(value_ref)?)
}

pub fn ecs_component_insert(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (entity_value_ref, component_value_ref): (JsValueRef, JsValueRef) =
        serde_json::from_value(args).context("parse args")?;

    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    // Get entity and make sure the entity exists
    let entity = entity_value_ref.get_entity(world, value_refs)?;
    world
        .get_entity(entity)
        .ok_or_else(|| format_err!("Entity does not exist"))?;

    let component_value_ref = value_refs
        .get(component_value_ref.key)
        .ok_or_else(|| format_err!("Value ref doesn't exist"))?
        .clone();

    // Load the type registry
    let type_registry = world.resource::<TypeRegistryArc>();
    let type_registry = type_registry.read();

    // Clone the reflect value of the component
    let reflect_value_ref = component_value_ref.get(world)?;
    let type_id = reflect_value_ref.type_id();
    let reflect_value = reflect_value_ref.clone_value();

    // Get the ReflectComponent
    let reflect_component = type_registry
        .get_type_data::<ReflectComponent>(type_id)
        .ok_or_else(|| format_err!("ReflectComponent not found for component value ref"))?
        .clone();

    // Drop our immutable borrow of the world
    drop(type_registry);
    drop(reflect_value_ref);

    // Add the component to the entity
    reflect_component.apply_or_insert(world, entity, reflect_value.as_reflect());

    Ok(serde_json::Value::Null)
}
