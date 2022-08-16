use crate::runtime::types::{Primitive, ReflectArgIntermediate, ReflectArgIntermediateValue};

use super::{
    v8_utils::{
        create_value_ref_object, reflect_value_ref_from_v8_value_transmit,
        reflect_value_ref_from_value_transmit, ValueRefObject,
    },
    WorldResource,
};
use bevy_ecs_dynamic::reflect_value_ref::ReflectValueRef;
use bevy_reflect_fns::{PassMode, ReflectArg};
use deno_core::{error::AnyError, op, serde_v8, v8, OpState, ResourceId};
use std::{any::TypeId, cell::RefCell, rc::Rc};

#[op(v8)]
fn op_value_ref_call(
    state: &mut OpState,
    scope: &mut v8::HandleScope,
    world_rid: ResourceId,
    value: serde_v8::Value<'_>,
    args: serde_v8::Value<'_>,
) -> Result<ValueRefObject<'static>, AnyError> {
    let world = state.resource_table.get::<WorldResource>(world_rid)?;
    let world = world.world.borrow();

    let (receiver, method) =
        unsafe { reflect_value_ref_from_value_transmit(scope, value) }?.method()?;

    // so collecting the arguments for the function call is a bit ugly, which is motivated by two
    // constraints:
    // 1. every argument needs to be passed as reference to the actual value (&[mut] dyn Reflect)
    // 2. "leaf types"/literals should be accepted and not need to be wrapped in a value ref
    //
    // Because of 1., the args all need to be collected into a vec (itermediate vec), and then another vec (arg vec)
    // is built containing references to the intermediate one, which is then passed to the function.
    // (`bevy_reflect_fns` could take a `&mut dyn Iterator` instead, which may be better).
    // And because we want to support literals, we need to downcast the values into the specific
    // types.
    // Primitives are downcast in the intermediate `map`, so we need a way to represent them and
    // pass them on to the arg vec, so we need a primitive enum.
    //
    // If anyone comes up with a better design, let me know.
    let v8_args: v8::Local<v8::Array> = args.v8_value.try_into().unwrap();
    let v8_args: Vec<_> = (0..v8_args.length())
        .map(|i| v8_args.get_index(scope, i).unwrap())
        .collect();

    let receiver_pass_mode = method.signature[0].0;
    let receiver_intermediate = match receiver_pass_mode {
        PassMode::Ref => ReflectArgIntermediateValue::Ref(receiver.get(&world)?),
        PassMode::RefMut => {
            unimplemented!("values passed by mutable reference in reflect fn call")
        }
        PassMode::Owned => ReflectArgIntermediateValue::Owned(receiver.get(&world)?),
    };
    let mut receiver_intermediate = ReflectArgIntermediate::Value(receiver_intermediate);

    let mut arg_intermediates = v8_args
        .into_iter()
        .zip(method.signature.iter().skip(1))
        .map(|(arg, &(pass_mode, type_id))| {
            let downcast_primitive = match type_id {
                type_id if type_id == TypeId::of::<f32>() => arg
                    .number_value(scope)
                    .map(|val| Primitive::f32(val as f32)),
                type_id if type_id == TypeId::of::<f64>() => arg
                    .number_value(scope)
                    .map(|val| Primitive::f64(val as f64)),
                type_id if type_id == TypeId::of::<i32>() => arg
                    .number_value(scope)
                    .map(|val| Primitive::i32(val as i32)),
                type_id if type_id == TypeId::of::<u32>() => arg
                    .number_value(scope)
                    .map(|val| Primitive::u32(val as u32)),
                _ => None,
            };
            if let Some(primitive) = downcast_primitive {
                return Ok(ReflectArgIntermediate::Primitive(primitive, pass_mode));
            }

            let value = unsafe { reflect_value_ref_from_v8_value_transmit(scope, arg) }?.value()?;
            let value = match pass_mode {
                PassMode::Ref => ReflectArgIntermediateValue::Ref(value.get(&world)?),
                // PassMode::RefMut => ReflectArgIntermediateValue::RefMut(value.get_mut_unchecked(&world)?),
                PassMode::RefMut => {
                    unimplemented!("values passed by mutable reference in reflect fn call")
                }
                PassMode::Owned => ReflectArgIntermediateValue::Owned(value.get(&world)?),
            };

            Ok(ReflectArgIntermediate::Value(value))
        })
        .collect::<Result<Vec<_>, AnyError>>()?;
    let mut args: Vec<ReflectArg> = std::iter::once(&mut receiver_intermediate)
        .chain(arg_intermediates.iter_mut())
        .map(|intermediate| intermediate.as_arg())
        .collect();

    let ret = method.call(args.as_mut_slice())?;
    let ret = Rc::new(RefCell::new(ret));
    let ret = ReflectValueRef::free(ret);

    Ok(unsafe { create_value_ref_object(scope, ret.into()) })
}
