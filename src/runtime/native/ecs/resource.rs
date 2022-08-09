use bevy_ecs_dynamic::reflect_value_ref::EcsValueRef;
use deno_core::{error::AnyError, op, v8, OpState, ResourceId};

use crate::runtime::WorldResource;

use super::{
    types::ComponentIdOrBevyType,
    v8_utils::{create_value_ref_object, ValueRefObject},
};

#[op(v8)]
pub fn op_world_get_resource(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    component_id: ComponentIdOrBevyType,
) -> Result<Option<ValueRefObject<'static>>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let world = world.world.borrow();

    let component_id = component_id.component_id(&world)?;

    // todo: implement world.contains_resource_by_id
    if world.get_resource_by_id(component_id).is_none() {
        return Ok(None);
    }
    let value_ref = EcsValueRef::resource(&world, component_id)?;

    Ok(Some(unsafe {
        create_value_ref_object(scope, value_ref.into())
    }))
}
