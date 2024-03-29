use std::{cell::RefCell, rc::Rc};

use anyhow::format_err;
use bevy::{
    ecs::component::{ComponentId, ComponentInfo},
    prelude::*,
};
use bevy_ecs_dynamic::reflect_value_ref::{
    EcsValueRef, ReflectValueRef, ReflectValueRefBorrow, ReflectValueRefBorrowMut,
};
use bevy_reflect::{Reflect, TypeRegistration, TypeRegistry};
use bevy_reflect_fns::{PassMode, ReflectArg, ReflectFunction};
use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

slotmap::new_key_type! {
    pub struct JsValueRefKey;
    pub struct ReflectFunctionKey;
}

/// Resource that stores [`ReflectValueRef`]s that are accessible to the JS runtime
#[derive(Default, Deref, DerefMut)]
pub struct JsValueRefs(SlotMap<JsValueRefKey, ReflectValueRef>);

/// Resource that stores [`ReflectFunction`]s that are accessible to the JS runtime
#[derive(Default, Deref, DerefMut)]
pub struct JsReflectFunctions(SlotMap<ReflectFunctionKey, ReflectFunction>);

#[derive(Serialize, Deserialize, Debug)]
pub struct JsValueRef {
    pub key: JsValueRefKey,
    pub function: Option<ReflectFunctionKey>,
}

impl JsValueRef {
    pub fn new_free(value: Box<dyn Reflect>, value_refs: &mut JsValueRefs) -> Self {
        let value = Rc::new(RefCell::new(value));
        let value = ReflectValueRef::free(value);

        JsValueRef {
            key: value_refs.insert(value),
            function: None,
        }
    }

    pub fn new_ecs(value: EcsValueRef, value_refs: &mut JsValueRefs) -> Self {
        JsValueRef {
            key: value_refs.insert(ReflectValueRef::ecs_ref(value)),
            function: None,
        }
    }

    /// If this value ref represents an [`Entity`] get it. Returns an error if it is not an entity.
    pub fn get_downcast_copy<T: Reflect + Copy>(
        &self,
        world: &World,
        value_refs: &JsValueRefs,
    ) -> anyhow::Result<T> {
        let value_ref: &ReflectValueRef = value_refs
            .get(self.key)
            .ok_or_else(|| format_err!("Value ref doesn't exist"))?;

        let borrow = value_ref.get(world)?;

        Ok(*borrow
            .downcast_ref()
            .ok_or_else(|| format_err!("Value passed not an entity"))?)
    }
}

#[derive(Serialize)]
pub struct JsQueryItem {
    pub entity: JsValueRef,
    pub components: Vec<JsValueRef>,
}

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
    pub fn component_id(
        &self,
        world: &World,
        type_registry: &TypeRegistry,
    ) -> Result<ComponentId, anyhow::Error> {
        match self {
            ComponentIdOrBevyType::ComponentId(id) => Ok(ComponentId::from(id)),
            ComponentIdOrBevyType::Type { type_name } => {
                let registration = type_registry.get_with_name(type_name).ok_or_else(|| {
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
    pub fn registration<'r>(
        &self,
        world: &World,
        type_registry: &'r TypeRegistry,
    ) -> Result<&'r TypeRegistration, anyhow::Error> {
        match self {
            ComponentIdOrBevyType::ComponentId(id) => {
                let component_id = ComponentId::from(id);
                let info = world.components().get_info(component_id).ok_or_else(|| {
                    anyhow::anyhow!(
                        "component `{component_id:?}` does not exist in the type registry"
                    )
                })?;
                let type_id = info.type_id().ok_or_else(|| {
                    anyhow::anyhow!("component `{component_id:?}` is not backed by a rust type",)
                })?;
                let registration = type_registry.get(type_id).ok_or_else(|| {
                    anyhow::anyhow!("`{component_id:?}` does not exist in the type registry")
                })?;

                Ok(registration)
            }
            ComponentIdOrBevyType::Type { type_name } => {
                let registration = type_registry.get_with_name(type_name).ok_or_else(|| {
                    anyhow::anyhow!("`{type_name}` does not exist in the type registry")
                })?;
                Ok(registration)
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

// Value, from which a `ReflectArg` can be borrowed
pub enum ReflectArgIntermediate<'a> {
    Value(ReflectArgIntermediateValue<'a>),
    Primitive(Primitive, PassMode),
}

pub enum ReflectArgIntermediateValue<'a> {
    Ref(ReflectValueRefBorrow<'a>),
    #[allow(dead_code)]
    RefMut(ReflectValueRefBorrowMut<'a>),
    Owned(ReflectValueRefBorrow<'a>),
}

impl<'a> ReflectArgIntermediateValue<'a> {
    pub fn as_arg(&mut self) -> ReflectArg<'_> {
        match self {
            ReflectArgIntermediateValue::Ref(val) => ReflectArg::Ref(&**val),
            ReflectArgIntermediateValue::RefMut(val) => ReflectArg::RefMut(&mut **val),
            ReflectArgIntermediateValue::Owned(val) => ReflectArg::Owned(&**val),
        }
    }
}
impl<'a> ReflectArgIntermediate<'a> {
    pub fn as_arg(&mut self) -> ReflectArg<'_> {
        match self {
            ReflectArgIntermediate::Value(val) => val.as_arg(),
            ReflectArgIntermediate::Primitive(prim, pass_mode) => prim.as_arg(*pass_mode),
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Primitive {
    f32(f32),
    f64(f64),
    i32(i32),
    u32(u32),
}

impl Primitive {
    pub fn as_arg(&mut self, pass_mode: PassMode) -> ReflectArg<'_> {
        let reflect: &mut dyn Reflect = match self {
            Primitive::f32(val) => val,
            Primitive::f64(val) => val,
            Primitive::i32(val) => val,
            Primitive::u32(val) => val,
        };

        match pass_mode {
            PassMode::Ref => ReflectArg::Ref(reflect),
            PassMode::RefMut => ReflectArg::RefMut(reflect),
            PassMode::Owned => ReflectArg::Owned(reflect),
        }
    }
}
