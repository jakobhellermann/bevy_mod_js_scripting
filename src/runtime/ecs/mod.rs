use super::WorldResource;
use deno_core::{include_js_files, Extension};

mod types;
mod v8_utils;

mod call;
mod info;
mod query;
mod value;

pub fn extension() -> Extension {
    Extension::builder()
        .ops(vec![
            info::op_world_tostring::decl(),
            info::op_world_components::decl(),
            info::op_world_resources::decl(),
            info::op_world_entities::decl(),
            query::op_world_query::decl(),
            value::op_value_ref_keys::decl(),
            value::op_value_ref_to_string::decl(),
            value::op_value_ref_get::decl(),
            value::op_value_ref_set::decl(),
            call::op_value_ref_call::decl(),
        ])
        .js(include_js_files!(prefix "bevy", "../js/ecs.js",))
        .build()
}
