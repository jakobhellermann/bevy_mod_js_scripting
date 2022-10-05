use anyhow::{format_err, Context};
use bevy::prelude::{default, Entity, ReflectComponent};

use crate::{JsValueRef, JsValueRefs, OpContext};

use super::types::ComponentIdOrBevyType;
pub fn ecs_entity_spawn(
    context: OpContext,
    world: &mut bevy::prelude::World,
    _args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let entity = world.spawn_empty().id();
    let value_ref = JsValueRef::new_free(Box::new(entity), value_refs);

    Ok(serde_json::to_value(value_ref)?)
}

pub fn ecs_component_insert(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (entity_value_ref, ty, component_value_ref): (
        JsValueRef,
        ComponentIdOrBevyType,
        JsValueRef,
    ) = serde_json::from_value(args).context("parse args")?;

    let registration = ty.registration(world, context.type_registry)?;

    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    // Get entity and make sure the entity exists
    let entity = entity_value_ref.get_downcast_copy::<Entity>(world, value_refs)?;
    world
        .get_entity(entity)
        .ok_or_else(|| format_err!("Entity does not exist"))?;

    let component_value_ref = value_refs
        .get(component_value_ref.key)
        .ok_or_else(|| format_err!("Value ref doesn't exist"))?
        .clone();

    // Clone the reflect value of the component
    let reflect_value = {
        let reflect_value_ref = component_value_ref.get(world)?;
        // clone it because it may
        reflect_value_ref.clone_value()
    };

    // Get the ReflectComponent
    let reflect_component = registration
        .data::<ReflectComponent>()
        .ok_or_else(|| format_err!("ReflectComponent not found for component value ref"))?
        .clone();

    // Add the component to the entity
    reflect_component.apply_or_insert(world, entity, &*reflect_value);

    Ok(serde_json::Value::Null)
}
