use bevy::{
    ecs::component::{ComponentId, ComponentInfo},
    prelude::{Entity, World},
};
use bevy_reflect::TypeRegistryArc;
use deno_core::error::AnyError;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ComponentIdOrBevyType {
    ComponentId(JsComponentId),
    Type {
        #[serde(rename = "typeName")]
        type_name: String,
    },
}

impl ComponentIdOrBevyType {
    pub fn component_id(self, world: &World) -> Result<ComponentId, AnyError> {
        match self {
            ComponentIdOrBevyType::ComponentId(id) => Ok(ComponentId::from(&id)),
            ComponentIdOrBevyType::Type { type_name } => {
                let type_registry = world.resource::<TypeRegistryArc>().read();
                let registration = type_registry.get_with_name(&type_name).ok_or_else(|| {
                    anyhow::anyhow!("`{type_name}` does not exist in the type registry")
                })?;
                let type_id = registration.type_id();
                let component_id = world
                    .components()
                    .get_id(type_id)
                    .or_else(|| world.components().get_resource_id(type_id))
                    .ok_or_else(|| anyhow::anyhow!("`{type_name}` is not a component"))?;
                Ok(component_id)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize)]
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

#[derive(Serialize)]
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
