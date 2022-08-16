use bevy::{ecs::component::ComponentId, prelude::*, utils::HashSet};
use wasm_bindgen::prelude::*;

use crate::runtime::{
    types::{JsComponentInfo, JsEntity},
    wasm::{BevyModJsScripting, WORLD_RID},
};

#[wasm_bindgen]
impl BevyModJsScripting {
    pub fn op_world_tostring(&self, rid: u32) -> String {
        assert_eq!(rid, WORLD_RID);
        let state = self.state();
        let world = &state.world;

        format!("{world:?}")
    }

    pub fn op_world_components(&self, rid: u32) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let state = self.state();
        let world = &state.world;

        let resource_components: HashSet<ComponentId> =
            world.archetypes().resource().components().collect();

        let infos = world
            .components()
            .iter()
            .filter(|info| !resource_components.contains(&info.id()))
            .map(JsComponentInfo::from)
            .collect::<Vec<_>>();

        Ok(serde_wasm_bindgen::to_value(&infos)?)
    }

    pub fn op_world_resources(&self, rid: u32) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let state = self.state();
        let world = &state.world;

        let infos = world
            .archetypes()
            .resource()
            .components()
            .map(|id| world.components().get_info(id).unwrap())
            .map(JsComponentInfo::from)
            .collect::<Vec<_>>();

        Ok(serde_wasm_bindgen::to_value(&infos)?)
    }

    pub fn op_world_entities(&self, rid: u32) -> Result<JsValue, JsValue> {
        assert_eq!(rid, WORLD_RID);
        let mut state = self.state();
        let world = &mut state.world;

        let entities = world
            .query::<Entity>()
            .iter(world)
            .map(JsEntity::from)
            .collect::<Vec<_>>();

        Ok(serde_wasm_bindgen::to_value(&entities)?)
    }
}
