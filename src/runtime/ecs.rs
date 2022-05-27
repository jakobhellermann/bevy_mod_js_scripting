use deno_core::{error::AnyError, include_js_files, op, Extension, OpState, ResourceId};

use super::WorldResource;

#[op]
fn op_world_tostring(state: &mut OpState, rid: ResourceId) -> Result<String, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    Ok(format!("{world:?}"))
}

pub fn extension() -> Extension {
    Extension::builder()
        .ops(vec![op_world_tostring::decl()])
        .js(include_js_files!(prefix "bevy", "js/ecs.js",))
        .build()
}
