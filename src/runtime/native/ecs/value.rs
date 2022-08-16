use super::{
    v8_utils::{
        create_value_ref_object, extend_local_lifetime, reflect_value_ref_from_value,
        reflect_value_ref_from_value_transmit, ReflectValueRefTransmit,
    },
    WorldResource,
};
use bevy_reflect::{ReflectRef, TypeRegistryArc};
use deno_core::{error::AnyError, op, serde_v8, v8, OpState, ResourceId};

macro_rules! try_downcast_leaf_get {
    ($value:ident with $scope:ident for $($ty:ty $(,)?),*) => {
        $(if let Some(value) = $value.downcast_ref::<$ty>() {
            let value = serde_v8::to_v8($scope, value)?;
            return Ok(unsafe { extend_local_lifetime(value).into() });
        })*
    };
}

#[op(v8)]
pub fn op_value_ref_get(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
    path: String,
) -> Result<serde_v8::Value<'static>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let world = world.world.borrow();

    let value_ref = unsafe { reflect_value_ref_from_value(scope, value) }?;

    let type_registry = world.resource::<TypeRegistryArc>();
    let type_registry = type_registry.read();

    let reflect_methods = type_registry
        .get_type_data::<bevy_reflect_fns::ReflectMethods>(value_ref.get(&world)?.type_id());
    if let Some(reflect_methods) = reflect_methods {
        let method_name = path.trim_start_matches('.');
        if let Some(reflect_function) = reflect_methods.get(method_name.trim_start_matches('.')) {
            return Ok(unsafe {
                create_value_ref_object(
                    scope,
                    ReflectValueRefTransmit::Method(value_ref.clone(), reflect_function.clone()),
                )
            });
        }
    }

    let value_ref = value_ref.append_path(&path, &world)?;
    {
        let value = value_ref.get(&world)?;
        try_downcast_leaf_get!(value with scope for
            u8, u16, u32, u64, u128, usize,
            i8, i16, i32, i64, i128, isize,
            String, char, bool, f32, f64
        );
    }

    let object = unsafe { create_value_ref_object(scope, value_ref.into()) };

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
pub fn op_value_ref_set(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
    path: String,
    new_value: serde_v8::Value<'_>,
) -> Result<(), AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let mut world = world.world.borrow_mut();

    let mut value =
        unsafe { reflect_value_ref_from_value(scope, value) }?.append_path(&path, &world)?;
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
pub fn op_value_ref_keys(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
) -> Result<Vec<String>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let world = world.world.borrow_mut();

    let value = unsafe { reflect_value_ref_from_value(scope, value) }?;
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
pub fn op_value_ref_to_string(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
) -> Result<String, AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let world = world.world.borrow();

    match unsafe { reflect_value_ref_from_value_transmit(scope, value)? } {
        ReflectValueRefTransmit::Value(value) => {
            let reflect = value.get(&world)?;
            Ok(format!("{reflect:?}"))
        }
        ReflectValueRefTransmit::Method(_, method) => Ok(method.fn_name.to_string()),
    }
}
