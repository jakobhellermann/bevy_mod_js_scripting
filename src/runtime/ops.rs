use super::OpMap;

mod ecs;
mod log;

pub fn get_core_ops() -> OpMap {
    let mut ops = OpMap::default();

    // Logging
    ops.insert("log", Box::new(log::OpLog));

    // ECS
    ecs::insert_ecs_ops(&mut ops);

    ops
}
