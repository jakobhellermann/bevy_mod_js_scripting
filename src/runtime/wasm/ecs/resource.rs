use bevy_ecs_dynamic::reflect_value_ref::{EcsValueRef, ReflectValueRef};
use wasm_bindgen::prelude::*;

use crate::runtime::{
    types::ComponentIdOrBevyType,
    wasm::{BevyModJsScripting, JsRuntimeState, JsValueRef, WORLD_RID},
    ToJsErr,
};

#[wasm_bindgen]
impl BevyModJsScripting {
    pub fn op_world_get_resource(
        &self,
        rid: u32,
        component_id: JsValue,
    ) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state();
        let JsRuntimeState {
            world, value_refs, ..
        } = &mut *state;

        let component_id: ComponentIdOrBevyType = serde_wasm_bindgen::from_value(component_id)?;
        let component_id = component_id.component_id(world).to_js_error()?;

        let value_ref = EcsValueRef::resource(world, component_id).to_js_error()?;

        let value_ref = JsValueRef {
            key: value_refs.insert(ReflectValueRef::ecs_ref(value_ref)),
            function: None,
        };

        Ok(serde_wasm_bindgen::to_value(&value_ref)?)
    }
}
