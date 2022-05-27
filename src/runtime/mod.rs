mod ecs;
mod log;

use std::{cell::RefCell, path::PathBuf};

use bevy::prelude::*;
use deno_core::{JsRuntime, ResourceId, RuntimeOptions};

struct WorldResource {
    world: RefCell<World>,
}
impl deno_core::Resource for WorldResource {}

const WORLD_RID: ResourceId = 0;

struct ScriptInfo {
    path: PathBuf,
}

pub fn create_runtime(path: PathBuf) -> JsRuntime {
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![ecs::extension(), log::extension()],
        ..Default::default()
    });

    let state = runtime.op_state();
    let mut state = state.borrow_mut();
    state.put(ScriptInfo { path });

    let rid = state.resource_table.add(WorldResource {
        world: RefCell::new(World::default()),
    });
    assert_eq!(rid, WORLD_RID);

    runtime
}

pub fn with_world<T>(
    world: &mut World,
    runtime: &mut JsRuntime,
    f: impl Fn(&mut JsRuntime) -> T,
) -> T {
    let resource = runtime
        .op_state()
        .borrow_mut()
        .resource_table
        .get::<WorldResource>(WORLD_RID)
        .unwrap();
    std::mem::swap(world, &mut *resource.world.borrow_mut());

    let ret = f(runtime);

    let resource = runtime
        .op_state()
        .borrow_mut()
        .resource_table
        .get::<WorldResource>(WORLD_RID)
        .unwrap();
    std::mem::swap(world, &mut *resource.world.borrow_mut());

    ret
}
