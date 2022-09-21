use super::OpMap;
use crate::JsRuntimeOp;

mod ecs;
mod log;

pub fn get_core_ops() -> OpMap {
    let mut ops = OpMap::default();

    // Logging
    ops.insert("log", Box::new(log::OpLog));

    // ECS
    ecs::insert_ecs_ops(&mut ops);

    // Type defs
    ops.insert("typedefs", Box::new(TypesJs));

    ops
}

struct TypesJs;
impl JsRuntimeOp for TypesJs {
    fn js(&self) -> Option<&'static str> {
        Some(include_str!("../../types/bevy_types.js"))
    }
}
