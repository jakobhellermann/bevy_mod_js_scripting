use bevy::{
    ecs::component::{ComponentId, ComponentInfo},
    prelude::Entity,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Deserialize, Serialize, Clone)]
pub struct JsComponentId {
    pub index: usize,
}
impl From<ComponentId> for JsComponentId {
    fn from(id: ComponentId) -> Self {
        JsComponentId { index: id.index() }
    }
}
impl From<&JsComponentId> for ComponentId {
    fn from(id: &JsComponentId) -> Self {
        ComponentId::new(id.index)
    }
}

#[derive(Deserialize, Serialize)]
pub struct JsComponentInfo {
    pub id: JsComponentId,
    pub name: String,
    pub size: usize,
}

impl From<&ComponentInfo> for JsComponentInfo {
    fn from(info: &ComponentInfo) -> Self {
        JsComponentInfo {
            id: info.id().into(),
            name: info.name().to_string(),
            size: info.layout().size(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct JsEntity {
    pub bits: u64,
    pub id: u32,
    pub generation: u32,
}
impl From<Entity> for JsEntity {
    fn from(entity: Entity) -> Self {
        JsEntity {
            bits: entity.to_bits(),
            id: entity.id(),
            generation: entity.generation(),
        }
    }
}
