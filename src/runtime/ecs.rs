use std::mem::ManuallyDrop;

use bevy::{
    ecs::component::{ComponentId, ComponentInfo},
    prelude::*,
    utils::HashSet,
};
use bevy_ecs_dynamic::reflect_value_ref::{query::EcsValueRefQuery, ReflectValueRef};
use bevy_reflect::ReflectRef;
use deno_core::{
    error::AnyError, include_js_files, op, serde_v8, v8, Extension, OpState, ResourceId,
};
use serde::{Deserialize, Serialize};

use super::WorldResource;

#[op]
fn op_world_tostring(state: &mut OpState, rid: ResourceId) -> Result<String, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    Ok(format!("{world:?}"))
}

#[derive(Serialize, Deserialize)]
struct JsComponentId {
    index: usize,
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
struct JsComponentInfo {
    id: JsComponentId,
    name: String,
    size: usize,
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
struct JsEntity {
    bits: u64,
    id: u32,
    generation: u32,
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

#[op]
fn op_world_components(
    state: &mut OpState,
    rid: ResourceId,
) -> Result<Vec<JsComponentInfo>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    let resource_components: HashSet<ComponentId> =
        world.archetypes().resource().components().collect();

    let infos = world
        .components()
        .iter()
        .filter(|info| !resource_components.contains(&info.id()))
        .map(JsComponentInfo::from)
        .collect();

    Ok(infos)
}

#[op]
fn op_world_resources(
    state: &mut OpState,
    rid: ResourceId,
) -> Result<Vec<JsComponentInfo>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    let infos = world
        .archetypes()
        .resource()
        .components()
        .map(|id| world.components().get_info(id).unwrap())
        .map(JsComponentInfo::from)
        .collect();

    Ok(infos)
}

#[op]
fn op_world_entities(state: &mut OpState, rid: ResourceId) -> Result<Vec<JsEntity>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let mut world = world.world.borrow_mut();

    let entities = world
        .query::<Entity>()
        .iter(&world)
        .map(JsEntity::from)
        .collect();

    Ok(entities)
}

#[derive(Deserialize)]
struct QueryDescriptor {
    components: Vec<JsComponentId>,
}

#[derive(Serialize)]
struct JsQueryItem {
    entity: JsEntity,
    components: Vec<ValueRefObject<'static>>,
}

#[op(v8)]
fn op_world_query(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    rid: ResourceId,
    descriptor: QueryDescriptor,
) -> Result<Vec<JsQueryItem>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(rid)?;
    let world = world.world.borrow_mut();

    let components: Vec<ComponentId> = descriptor
        .components
        .iter()
        .map(ComponentId::from)
        .collect();

    let mut query = EcsValueRefQuery::new(&world, &components);

    let results = query
        .iter(&world)
        .map(|item| {
            let components = item
                .items
                .into_iter()
                .map(|value| unsafe {
                    create_value_ref_object(scope, ReflectValueRef::ecs_ref(value))
                })
                .collect();

            JsQueryItem {
                entity: item.entity.into(),
                components,
            }
        })
        .collect();

    Ok(results)
}

type ValueRefObject<'a> = serde_v8::Value<'a>;

unsafe fn reflect_value_ref_from_value<'a>(
    scope: &mut v8::HandleScope,
    value: ValueRefObject<'a>,
) -> &'a ReflectValueRef {
    let value: v8::Local<v8::Value> = value.into();
    let value = value.to_object(scope).unwrap();
    let external = value.get_internal_field(scope, 0).unwrap();
    assert!(external.is_external());
    let external = v8::Local::<v8::External>::cast(external);
    let value = &*external.value().cast::<ReflectValueRef>();

    value
}

macro_rules! try_downcast_leaf_get {
    ($value:ident with $scope:ident for $($ty:ty $(,)?),*) => {
        $(if let Some(value) = $value.downcast_ref::<$ty>() {
            let value = serde_v8::to_v8($scope, value)?;
            return Ok(unsafe { extend_local_lifetime(value).into() });
        })*
    };
}

#[op(v8)]
fn op_value_ref_get(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
    path: String,
) -> Result<serde_v8::Value<'static>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let world = world.world.borrow_mut();

    let value_ref = unsafe { reflect_value_ref_from_value(scope, value) };
    let value_ref = value_ref.append_path(&path, &world)?;
    let value = value_ref.get(&world)?;
    try_downcast_leaf_get!(value with scope for
        u8, u16, u32, u64, u128, usize,
        i8, i16, i32, i64, i128, isize,
        String, char, bool, f32, f64
    );

    let object = unsafe { create_value_ref_object(scope, value_ref.clone()) };

    Ok(object)
}

macro_rules! try_downcast_leaf_set {
    ($value:ident <- $new_value:ident with $scope:ident for $($ty:ty $(,)?),*) => {
        $(if let Some(value) = $value.downcast_mut::<$ty>() {
            *value = serde_v8::from_v8($scope, $new_value.v8_value)?;
            return Ok(());
        })*
    };
}

#[op(v8)]
fn op_value_ref_set(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
    path: String,
    new_value: serde_v8::Value<'_>,
) -> Result<(), AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let mut world = world.world.borrow_mut();

    let mut value = unsafe { reflect_value_ref_from_value(scope, value).append_path(&path, &world)? };
    let mut value = value.get_mut(&mut world)?;

    try_downcast_leaf_set!(value <- new_value with scope for
        u8, u16, u32, u64, u128, usize,
        i8, i16, i32, i64, i128, isize,
        String, char, bool, f32, f64
    );

    Err(anyhow::anyhow!(
        "could not set value reference: type `{}` is not a primitive type",
        value.type_name()
    ))
}

#[op(v8)]
fn op_value_ref_keys(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
) -> Result<Vec<String>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let world = world.world.borrow_mut();

    let value = unsafe { reflect_value_ref_from_value(scope, value) };
    let reflect = value.get(&world)?;

    let fields = match reflect.reflect_ref() {
        ReflectRef::Struct(s) => (0..s.field_len())
            .map(|i| {
                let name = s.name_at(i).ok_or_else(|| {
                    anyhow::anyhow!("misbehaving Reflect impl on `{}`", s.type_name())
                })?;
                Ok(name.to_owned())
            })
            .collect::<Result<_, AnyError>>()?,
        ReflectRef::Tuple(tuple) => (0..tuple.field_len()).map(|i| i.to_string()).collect(),
        ReflectRef::TupleStruct(tuple_struct) => (0..tuple_struct.field_len())
            .map(|i| i.to_string())
            .collect(),
        _ => Vec::new(),
    };

    Ok(fields)
}

#[op(v8)]
fn op_value_ref_to_string(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
) -> Result<String, AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let world = world.world.borrow_mut();

    let value = unsafe { reflect_value_ref_from_value(scope, value) };
    let reflect = value.get(&world)?;

    Ok(format!("{reflect:?}"))
}

unsafe fn create_value_ref_object(
    scope: &mut v8::HandleScope,
    value_ref: ReflectValueRef,
) -> ValueRefObject<'static> {
    let object = create_object_with_dropped_internal(scope, value_ref);
    let object: v8::Local<v8::Value> = object.into();
    let object = extend_local_lifetime(object);
    object.into()
}

fn create_object_with_dropped_internal<'s, T: 'static>(
    scope: &'s mut v8::HandleScope,
    value: T,
) -> v8::Local<'s, v8::Object> {
    let template = v8::ObjectTemplate::new(scope);
    assert!(template.set_internal_field_count(1));

    let instance = template.new_instance(scope).unwrap();

    let external = create_dropped_external(scope, instance, value);
    assert!(instance.set_internal_field(0, external.into()));

    instance
}

fn create_dropped_external<'s, T: 'static, D>(
    scope: &'s mut v8::HandleScope,
    handle: impl v8::Handle<Data = D>,
    value: T,
) -> v8::Local<'s, v8::External> {
    let ptr = Box::into_raw(Box::new(value));

    let external = v8::External::new(scope, ptr.cast());

    schedule_finalizer(scope, handle, move |_| {
        unsafe { std::mem::drop(Box::from_raw(ptr)) };
    });

    external
}

fn schedule_finalizer<D>(
    scope: &mut v8::HandleScope,
    handle: impl v8::Handle<Data = D>,
    finalizer: impl FnOnce(&mut v8::Isolate) + 'static,
) {
    let weak = v8::Weak::with_finalizer(
        scope,
        handle,
        Box::new(move |isolate| {
            finalizer(isolate);
        }),
    );
    let _ = ManuallyDrop::new(weak);
}

unsafe fn extend_local_lifetime<'a, 'b, T>(val: v8::Local<'a, T>) -> v8::Local<'b, T> {
    std::mem::transmute(val)
}

pub fn extension() -> Extension {
    Extension::builder()
        .ops(vec![
            op_world_tostring::decl(),
            op_world_components::decl(),
            op_world_resources::decl(),
            op_world_entities::decl(),
            op_world_query::decl(),
            op_value_ref_keys::decl(),
            op_value_ref_to_string::decl(),
            op_value_ref_get::decl(),
            op_value_ref_set::decl(),
        ])
        .js(include_js_files!(prefix "bevy", "js/ecs.js",))
        .build()
}
