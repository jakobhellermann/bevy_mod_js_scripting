use bevy::ecs::component::ComponentId;
use bevy_ecs_dynamic::reflect_value_ref::{query::EcsValueRefQuery, ReflectValueRef};
use wasm_bindgen::prelude::*;

use crate::runtime::{
    types::QueryDescriptor,
    wasm::{BevyModJsScripting, JsQueryItem, JsRuntimeState, JsValueRef, WORLD_RID},
};

#[wasm_bindgen]
impl BevyModJsScripting {
    pub fn op_world_query(&self, rid: u32, query: JsValue) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state();
        let JsRuntimeState {
            world, value_refs, ..
        } = &mut *state;

        let descriptor: QueryDescriptor = serde_wasm_bindgen::from_value(query)?;

        let components: Vec<ComponentId> = descriptor
            .components
            .iter()
            .map(ComponentId::from)
            .collect();

        let mut query = EcsValueRefQuery::new(world, &components);
        let results = query
            .iter(world)
            .map(|item| {
                let components = item
                    .items
                    .into_iter()
                    .map(|value| JsValueRef {
                        key: value_refs.insert(ReflectValueRef::ecs_ref(value)),
                        function: None,
                    })
                    .collect();

                JsQueryItem {
                    entity: item.entity.into(),
                    components,
                }
            })
            .collect::<Vec<_>>();

        Ok(serde_wasm_bindgen::to_value(&results)?)
    }
}
