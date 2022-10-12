use std::any::TypeId;

use anyhow::{bail, format_err, Context};
use bevy::{
    prelude::{default, ReflectDefault, World},
    utils::HashMap,
};
use bevy_ecs_dynamic::reflect_value_ref::ReflectValueRef;
use bevy_reflect::{Reflect, ReflectRef, TypeRegistryArc};
use bevy_reflect_fns::{PassMode, ReflectArg, ReflectMethods};

use crate::{runtime::OpContext, JsRuntimeOp, JsReflectFunctions};

use super::{
    types::{
        JsValueRef, JsValueRefs, Primitive, ReflectArgIntermediate, ReflectArgIntermediateValue,
    },
    WithValueRefs,
};

macro_rules! try_downcast_leaf_get {
    ($value:ident for $($ty:ty $(,)?),*) => {
        (|| {
            $(if let Some(value) = $value.downcast_ref::<$ty>() {
                let value = serde_json::to_value(value)?;
                return Ok(Some(value));
            })*

            Ok::<_, anyhow::Error>(None)
        })()
    };
}

macro_rules! try_downcast_leaf_set {
    ($value:ident <- $new_value:ident for $($ty:ty $(,)?),*) => {
        (|| {
            $(if let Some(value) = $value.downcast_mut::<$ty>() {
                *value = serde_json::from_value($new_value)?;
                return Ok(());
            })*

            bail!("Couldn't assign to primitive");
        })()
    };
}

#[derive(Debug)]
pub enum JsonValueOrReflect {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<JsonValueOrReflect>),
    Object(HashMap<String, JsonValueOrReflect>),
    Reflect(Box<dyn Reflect>),
}

impl JsonValueOrReflect {
    fn into_primitive_value(self) -> Option<serde_json::Value> {
        use serde_json::Value as V;
        Some(match self {
            JsonValueOrReflect::Null => V::Null,
            JsonValueOrReflect::Bool(b) => V::Bool(b),
            JsonValueOrReflect::Number(n) => V::Number(n),
            JsonValueOrReflect::String(s) => V::String(s),
            _ => return None,
        })
    }
    fn from_value(
        value: serde_json::Value,
        value_refs: &JsValueRefs,
        world: &World,
    ) -> anyhow::Result<Self> {
        if let Ok(value_ref) = serde_json::from_value::<JsValueRef>(value.clone()) {
            let value_ref = value_refs
                .get(value_ref.key)
                .ok_or_else(|| format_err!("Value ref doesn't exist"))?;
            let reflect = value_ref.get(world)?.clone_value();

            Ok(Self::Reflect(reflect))
        } else {
            match value {
                serde_json::Value::Null => Ok(Self::Null),
                serde_json::Value::Bool(b) => Ok(Self::Bool(b)),
                serde_json::Value::Number(n) => Ok(Self::Number(n)),
                serde_json::Value::String(s) => Ok(Self::String(s)),
                serde_json::Value::Array(arr) => Ok(Self::Array(
                    arr.into_iter()
                        .map(|value| Self::from_value(value, value_refs, world))
                        .collect::<Result<_, _>>()?,
                )),
                serde_json::Value::Object(map) => {
                    let mut object = HashMap::default();
                    for (key, value) in map {
                        object.insert(key, Self::from_value(value, value_refs, world)?);
                    }
                    Ok(Self::Object(object))
                }
            }
        }
    }
}

/// Converts a JSON value to a dynamic reflect struct or list
pub fn patch_reflect_with_json(
    value: &mut dyn Reflect,
    patch: JsonValueOrReflect,
) -> anyhow::Result<()> {
    match patch {
        JsonValueOrReflect::Reflect(patch) => {
            if !reflect_is_compatible(value, patch.as_reflect()) {
                bail!(
                    "Cannot assign type {} to {}",
                    value.type_name(),
                    patch.type_name()
                );
            }
            value.apply(patch.as_reflect());
        }
        JsonValueOrReflect::Null => {
            bail!("Can't patch values with null");
        }
        patch @ (JsonValueOrReflect::Bool(_)
        | JsonValueOrReflect::Number(_)
        | JsonValueOrReflect::String(_)) => {
            let patch = patch.into_primitive_value().unwrap();
            try_downcast_leaf_set!(value <- patch for
                u8, u16, u32, u64, u128, usize,
                i8, i16, i32, i64, i128, isize,
                String, char, bool, f32, f64
            )?;
        }
        JsonValueOrReflect::Array(array) => match value.reflect_mut() {
            bevy_reflect::ReflectMut::Struct(_) => bail!("Cannot patch struct with Array"),
            bevy_reflect::ReflectMut::List(target) => {
                let target_len = target.len();
                let patch_len = array.len();
                if target_len < patch_len {
                    bail!("Cannot patch list with {target_len} elements with patch with {patch_len} elements");
                }

                for (i, patch) in array.into_iter().enumerate() {
                    let target = target.get_mut(i).unwrap();
                    patch_reflect_with_json(target, patch)?;
                }
            }
            bevy_reflect::ReflectMut::Tuple(target) => {
                let target_len = target.field_len();
                let patch_len = array.len();
                if target_len != patch_len {
                    bail!("Cannot patch tuple with {target_len} elements with patch with {patch_len} elements");
                }

                for (i, patch) in array.into_iter().enumerate() {
                    let target = target.field_mut(i).unwrap();
                    patch_reflect_with_json(target, patch)?;
                }
            }
            bevy_reflect::ReflectMut::TupleStruct(target) => {
                let target_len = target.field_len();
                let patch_len = array.len();
                if target_len != patch_len {
                    bail!("Cannot patch tuple with {target_len} elements with patch with {patch_len} elements");
                }

                for (i, patch) in array.into_iter().enumerate() {
                    let target = target.field_mut(i).unwrap();
                    patch_reflect_with_json(target, patch)?;
                }
            }
            bevy_reflect::ReflectMut::Array(target) => {
                let target_len = target.len();
                let patch_len = array.len();
                if target_len != patch_len {
                    bail!("Cannot patch array with {target_len} elements with patch with {patch_len} elements");
                }

                for (i, patch) in array.into_iter().enumerate() {
                    let target = target.get_mut(i).unwrap();
                    patch_reflect_with_json(target, patch)?;
                }
            }
            bevy_reflect::ReflectMut::Map(_) => bail!("Cannot patch map with array"),
            bevy_reflect::ReflectMut::Value(_) => bail!("Cannot patch primitive value with array"),
        },
        JsonValueOrReflect::Object(map) => match value.reflect_mut() {
            bevy_reflect::ReflectMut::Struct(target) => {
                for (key, value) in map {
                    let field = target.field_mut(&key).ok_or_else(|| {
                        format_err!("Field `{key}` in patch does not exist on target struct")
                    })?;

                    patch_reflect_with_json(field, value)?;
                }
            }
            bevy_reflect::ReflectMut::Map(_) => {
                bail!("Patching maps are not yet supported");
                // TODO: The code would be something like below, but we have to figure out how to
                // insert new values of the right type, or find out that it isn't actually a concern.

                // for (key, value) in map {
                //     let key = Box::new(key) as Box<dyn Reflect>;
                //     if let Some(field) = target.get_mut(key.as_reflect()) {
                //         patch_reflect_with_json(field, value)?;
                //     } else {
                //         target.insert_boxed(
                //             key,
                //             /* How do we know what the expected value type for the map is? */
                //         );
                //     }
                // }
            }
            bevy_reflect::ReflectMut::Tuple(_) | bevy_reflect::ReflectMut::TupleStruct(_) => {
                bail!("Cannot patch tuple struct with object")
            }
            bevy_reflect::ReflectMut::List(_) | bevy_reflect::ReflectMut::Array(_) => {
                bail!("Cannot patch list or array with object")
            }
            bevy_reflect::ReflectMut::Value(_) => bail!("Cannot patch primitive value with object"),
        },
    }

    Ok(())
}

/// Check whether or not it's safe to `Reflect.apply` one reflect to another
fn reflect_is_compatible(reflect1: &dyn Reflect, reflect2: &dyn Reflect) -> bool {
    match (reflect1.reflect_ref(), reflect2.reflect_ref()) {
        (ReflectRef::Value(value1), ReflectRef::Value(value2)) => {
            value1.type_id() == value2.type_id()
        }
        (ReflectRef::Array(arr1), ReflectRef::Array(arr2)) => {
            arr1.iter()
                .zip(arr2.iter())
                .fold(true, |compatible, (reflect1, reflect2)| {
                    compatible && reflect_is_compatible(reflect1, reflect2)
                })
        }
        (ReflectRef::List(list1), ReflectRef::List(list2)) => {
            list1
                .iter()
                .zip(list2.iter())
                .fold(true, |compatible, (reflect1, reflect2)| {
                    compatible && reflect_is_compatible(reflect1, reflect2)
                })
        }
        (ReflectRef::Tuple(tuple1), ReflectRef::Tuple(tuple2)) => tuple1
            .iter_fields()
            .zip(tuple2.iter_fields())
            .fold(true, |compatible, (reflect1, reflect2)| {
                compatible && reflect_is_compatible(reflect1, reflect2)
            }),
        (ReflectRef::TupleStruct(tuple1), ReflectRef::TupleStruct(tuple2)) => tuple1
            .iter_fields()
            .zip(tuple2.iter_fields())
            .fold(true, |compatible, (reflect1, reflect2)| {
                compatible && reflect_is_compatible(reflect1, reflect2)
            }),
        (ReflectRef::Struct(struct1), ReflectRef::Struct(struct2)) => struct1
            .iter_fields()
            .enumerate()
            .fold(true, |compatible, (i, field1)| {
                if let Some(field2) = struct2.field(struct1.name_at(i).unwrap()) {
                    compatible && reflect_is_compatible(field1, field2)
                } else {
                    compatible
                }
            }),
        _ => false,
    }
}

pub fn ecs_value_ref_get(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    context
        .op_state
        .with_refs_and_funcs(|_, value_refs, reflect_functions| {
            // Parse args
            let (value_ref, path): (JsValueRef, String) =
                serde_json::from_value(args).context("parse args")?;

            // Load the type registry
            let type_registry = world.resource::<TypeRegistryArc>();
            let type_registry = type_registry.read();

            // Get the reflect value ref from the JS argument
            let value_ref = value_refs
                .get(value_ref.key)
                .ok_or_else(|| format_err!("Value ref doesn't exist"))?
                .clone();

            // See if we can find any reflect methods for this type in the type registry
            let reflect_methods =
                type_registry.get_type_data::<ReflectMethods>(value_ref.get(world)?.type_id());

            // If we found methods for this type
            if let Some(reflect_methods) = reflect_methods {
                let method_name = &path;
                // If the path we are accessing is a method on the type
                if let Some(reflect_function) = reflect_methods.get(method_name) {
                    // Return a method reference
                    let value = JsValueRef {
                        key: value_refs.insert(value_ref),
                        function: Some(reflect_functions.insert(reflect_function.clone())),
                    };

                    return Ok(serde_json::to_value(&value)?);
                }
            }

            // If we didn't find a method, add the path to our value ref
            let value_ref = append_path(value_ref, path, world)?;

            // Try to downcast the value to a primitive
            {
                let value = value_ref.get(world)?;

                let value = try_downcast_leaf_get!(value for
                    u8, u16, u32, u64, u128, usize,
                    i8, i16, i32, i64, i128, isize,
                    String, char, bool, f32, f64
                );

                if let Some(value) = value? {
                    return Ok(value);
                }
            }

            // If not a primitive, just return a new value ref
            let object = JsValueRef {
                key: value_refs.insert(value_ref),
                function: None,
            };

            Ok(serde_json::to_value(object)?)
        })
}

pub fn ecs_value_ref_set(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (value_ref, path, new_value): (JsValueRef, String, serde_json::Value) =
        serde_json::from_value(args).context("parse args")?;

    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    // Get the value ref from the JS arg
    let value_ref = value_refs
        .get(value_ref.key)
        .ok_or_else(|| format_err!("Value ref doesn't exist"))?
        .clone();

    // Access the provided path on the value ref
    let mut value_ref = append_path(value_ref, path, world)?;

    // Try to asign as a primitive
    let primitive_assignment_result = {
        let new_value = new_value.clone();
        let mut reflect = value_ref.get_mut(world)?;

        // Try to store a primitive in the value
        try_downcast_leaf_set!(reflect <- new_value for
            u8, u16, u32, u64, u128, usize,
            i8, i16, i32, i64, i128, isize,
            String, char, bool, f32, f64
        )
        .map_err(|e| {
            format_err!(
                "could not set value reference: type `{type_name}` is not a primitive \
                type or value ref: {e}",
                type_name = reflect.type_name(),
            )
        })
    };

    // If we could not assign a primitive
    if let Err(e) = primitive_assignment_result {
        // Try to assign as a reflect value
        if let Ok(new_js_value_ref) = serde_json::from_value::<JsValueRef>(new_value) {
            let new_value_ref = value_refs
                .get(new_js_value_ref.key)
                .ok_or_else(|| format_err!("Value ref doesn't exist"))?;
            let new_reflect = new_value_ref.get(world)?.clone_value();
            let mut reflect = value_ref.get_mut(world)?;

            if !reflect_is_compatible(new_reflect.as_reflect(), reflect.as_reflect()) {
                bail!(
                    "Cannot assign value ref. {} cannot be assigned to {}",
                    new_reflect.type_name(),
                    reflect.type_name()
                );
            }

            reflect.apply(new_reflect.as_reflect());

            Ok(serde_json::Value::Null)
        } else {
            Err(e)
        }
    } else {
        Ok(serde_json::Value::Null)
    }
}

pub fn ecs_value_ref_keys(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (value_ref,): (JsValueRef,) = serde_json::from_value(args).context("parse args")?;

    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    // Get the value ref from the JS arg
    let value_ref = value_refs
        .get(value_ref.key)
        .ok_or_else(|| format_err!("Value ref doesn't exist"))?
        .clone();
    let reflect = value_ref.get(world).unwrap();

    // Enumerate the fields of the reflected object
    let fields = match reflect.reflect_ref() {
        ReflectRef::Struct(s) => (0..s.field_len())
            .map(|i| {
                let name = s.name_at(i).ok_or_else(|| {
                    format_err!("misbehaving Reflect impl on `{}`", s.type_name())
                })?;
                Ok(name.to_owned())
            })
            .collect::<anyhow::Result<_>>()?,
        ReflectRef::Tuple(tuple) => (0..tuple.field_len()).map(|i| i.to_string()).collect(),
        ReflectRef::TupleStruct(tuple_struct) => (0..tuple_struct.field_len())
            .map(|i| i.to_string())
            .collect(),
        _ => Vec::new(),
    };

    Ok(serde_json::to_value(fields)?)
}

pub fn ecs_value_ref_to_string(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (value_ref,): (JsValueRef,) = serde_json::from_value(args).context("parse args")?;

    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    // Get the value ref from the JS arg
    let value_ref = value_refs
        .get(value_ref.key)
        .ok_or_else(|| format_err!("Value ref doesn't exist"))?
        .clone();
    let reflect = value_ref.get(world).unwrap();

    Ok(serde_json::Value::String(format!("{reflect:?}")))
}

pub fn ecs_value_ref_default(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (type_name, patch): (String, Option<serde_json::Value>) =
        serde_json::from_value(args).context("parse args")?;

    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let patch = patch
        .map(|patch| JsonValueOrReflect::from_value(patch, value_refs, world))
        .transpose()?;

    // Load the type registry
    let type_registry = world.resource::<TypeRegistryArc>();
    let type_registry = type_registry.read();

    // Get the type registration for the named type
    let type_registration = type_registry
        .get_with_name(&type_name)
        .ok_or_else(|| format_err!("Type not registered: {type_name}"))?;

    // Get the default creator for the reflected type
    let reflect_default = type_registration
        .data::<ReflectDefault>()
        .ok_or_else(|| format_err!("Type does not have ReflectDefault: {type_name}"))?;
    let mut value = reflect_default.default();

    // Patch the default value if a patch is provided
    if let Some(patch) = patch {
        patch_reflect_with_json(value.as_reflect_mut(), patch)?;
    }

    // Return the value ref to the new object
    let value_ref = JsValueRef::new_free(value, value_refs);
    Ok(serde_json::to_value(value_ref)?)
}

pub fn ecs_value_ref_patch(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (value_ref, patch): (JsValueRef, serde_json::Value) =
        serde_json::from_value(args).context("parse args")?;

    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    let patch = JsonValueOrReflect::from_value(patch, value_refs, world)?;

    let value_ref = value_refs
        .get_mut(value_ref.key)
        .ok_or_else(|| format_err!("Value ref does not exist"))?;

    let mut value = value_ref.get_mut(world)?;

    // Patch the default value if a patch is provided
    patch_reflect_with_json(value.as_reflect_mut(), patch)?;

    Ok(serde_json::Value::Null)
}

pub fn ecs_value_ref_call(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (receiver, args): (JsValueRef, Vec<serde_json::Value>) =
        serde_json::from_value(args).context("parse args")?;

    let ref_not_exist_err = || format_err!("Ref does not exist");

    context
        .op_state
        .with_refs_and_funcs(|_, value_refs, reflect_functions| {
            // Get the receiver's reflect_function
            let method_key = receiver
                .function
                .ok_or_else(|| format_err!("Cannot call non-function ref"))?;
            let method = reflect_functions
                .get_mut(method_key)
                .ok_or_else(ref_not_exist_err)?;

            // Get the receiver's reflect ref
            let receiver = value_refs.get(receiver.key).ok_or_else(ref_not_exist_err)?;

            // Collect the receiver intermediate value
            let receiver_pass_mode = method.signature[0].0;
            let receiver_intermediate = match receiver_pass_mode {
                PassMode::Ref => ReflectArgIntermediateValue::Ref(receiver.get(world).unwrap()),
                PassMode::RefMut => {
                    unimplemented!("values passed by mutable reference in reflect fn call")
                }
                PassMode::Owned => ReflectArgIntermediateValue::Owned(receiver.get(world).unwrap()),
            };
            let mut receiver_intermediate = ReflectArgIntermediate::Value(receiver_intermediate);

            // Collect the intermediate values for the arguments
            let mut arg_intermediates = args
                .iter()
                .zip(method.signature.iter().skip(1))
                .map(|(arg, &(pass_mode, type_id))| {
                    // Try to cast the arg as a primitive
                    let downcast_primitive = match type_id {
                        type_id if type_id == TypeId::of::<f32>() => {
                            Some(Primitive::f32(serde_json::from_value(arg.clone())?))
                        }
                        type_id if type_id == TypeId::of::<f64>() => {
                            Some(Primitive::f64(serde_json::from_value(arg.clone())?))
                        }
                        type_id if type_id == TypeId::of::<i32>() => {
                            Some(Primitive::i32(serde_json::from_value(arg.clone())?))
                        }
                        type_id if type_id == TypeId::of::<u32>() => {
                            Some(Primitive::u32(serde_json::from_value(arg.clone())?))
                        }
                        _ => None,
                    };
                    // If the arg cast worked, return a primitive arg
                    if let Some(primitive) = downcast_primitive {
                        return Ok(ReflectArgIntermediate::Primitive(primitive, pass_mode));
                    }

                    // Otherwise, try get the arg as a value ref
                    let value_ref: JsValueRef = serde_json::from_value(arg.clone())?;
                    let value_ref = value_refs
                        .get(value_ref.key)
                        .ok_or_else(|| format_err!("Value ref doesn't exist"))?;

                    let value_ref = match pass_mode {
                        PassMode::Ref => {
                            ReflectArgIntermediateValue::Ref(value_ref.get(world).unwrap())
                        }
                        PassMode::RefMut => {
                            unimplemented!("values passed by mutable reference in reflect fn call")
                        }
                        PassMode::Owned => {
                            ReflectArgIntermediateValue::Owned(value_ref.get(world).unwrap())
                        }
                    };

                    Ok(ReflectArgIntermediate::Value(value_ref))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;

            // Collect references to our intermediates as [`ReflectArg`]s
            let mut args: Vec<ReflectArg> = std::iter::once(&mut receiver_intermediate)
                .chain(arg_intermediates.iter_mut())
                .map(|intermediate| intermediate.as_arg())
                .collect();

            // Finally call the method
            let ret = method.call(args.as_mut_slice()).unwrap();

            // Drop our intermediates and args so that we can use `value_refs` again, below.
            drop(args);
            drop(arg_intermediates);
            drop(receiver_intermediate);

            let ret = JsValueRef::new_free(ret, value_refs);

            Ok(serde_json::to_value(ret)?)
        })
}

pub fn ecs_value_ref_eq(
    context: OpContext,
    world: &mut bevy::prelude::World,
    args: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Parse args
    let (ref1, ref2): (JsValueRef, JsValueRef) =
        serde_json::from_value(args).context("parse args")?;

    let value_refs = context
        .op_state
        .entry::<JsValueRefs>()
        .or_insert_with(default);

    // Get the value ref from the JS arg
    let ref1 = value_refs
        .get(ref1.key)
        .ok_or_else(|| format_err!("Value ref doesn't exist"))?
        .clone();
    let reflect1 = ref1.get(world).unwrap();

    let ref2 = value_refs
        .get(ref2.key)
        .ok_or_else(|| format_err!("Value ref doesn't exist"))?
        .clone();
    let reflect2 = ref2.get(world).unwrap();

    Ok(serde_json::Value::Bool(
        reflect1
            .as_reflect()
            .reflect_partial_eq(reflect2.as_reflect())
            .unwrap_or(false),
    ))
}

pub struct EcsValueRefCleanup;

impl JsRuntimeOp for EcsValueRefCleanup {
    fn frame_end(&self, op_state: &mut type_map::TypeMap, _: &mut World) {
        op_state
            .entry::<JsValueRefs>()
            .or_insert_with(default)
            .clear();
        op_state
            .entry::<JsReflectFunctions>()
            .or_insert_with(default)
            .clear();
    }
}

fn append_path(
    value_ref: ReflectValueRef,
    path: String,
    world: &World,
) -> Result<ReflectValueRef, anyhow::Error> {
    let value = value_ref.get(world)?;
    let path = match value.reflect_ref() {
        ReflectRef::Struct(_) | ReflectRef::TupleStruct(_) | ReflectRef::Tuple(_) => {
            format!(".{path}")
        }
        ReflectRef::List(_) | ReflectRef::Array(_) => format!("[{path}]"),
        ReflectRef::Map(_) | ReflectRef::Value(_) => path,
    };
    Ok(value_ref.append_path(&path, world)?)
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Reflect, Default)]
    struct S1 {
        a: String,
        b: f32,
    }

    #[derive(Reflect, Default)]
    struct S2 {
        a: String,
        b: f32,
        c: u32,
    }

    #[derive(Reflect, Default)]
    struct S3 {
        a: String,
        b: u32,
    }

    #[test]
    fn test_reflect_is_compatible_check() {
        let string = Box::new(String::default()) as Box<dyn Reflect>;
        let uint = Box::new(0u32) as Box<dyn Reflect>;
        let mut s1 = Box::new(S1::default()) as Box<dyn Reflect>;
        let s2 = Box::new(S2::default()) as Box<dyn Reflect>;
        let s3 = Box::new(S3::default()) as Box<dyn Reflect>;

        assert!(!reflect_is_compatible(
            uint.as_reflect(),
            string.as_reflect()
        ));
        assert!(!reflect_is_compatible(s1.as_reflect(), string.as_reflect()));

        assert!(reflect_is_compatible(s1.as_reflect(), s2.as_reflect()));
        s1.apply(s2.as_reflect());

        assert!(!reflect_is_compatible(s1.as_reflect(), s3.as_reflect()));

        let mut l1 = Box::new(vec![1, 2, 3]) as Box<dyn Reflect>;
        let l2 = Box::new(vec![5, 4, 3, 2, 1]) as Box<dyn Reflect>;

        assert!(reflect_is_compatible(l1.as_reflect(), l2.as_reflect()));
        l1.apply(l2.as_reflect());
        assert!(!reflect_is_compatible(l1.as_reflect(), s1.as_reflect()));

        let mut t1 = Box::new((0u32, String::from("hi"))) as Box<dyn Reflect>;
        let t2 = Box::new((1u32, String::from("bye"))) as Box<dyn Reflect>;
        let t3 = Box::new((0f32, String::from("bye"))) as Box<dyn Reflect>;

        assert!(reflect_is_compatible(t1.as_reflect(), t2.as_reflect()));
        t1.apply(t2.as_reflect());
        assert!(!reflect_is_compatible(t1.as_reflect(), t3.as_reflect()));
    }
}
