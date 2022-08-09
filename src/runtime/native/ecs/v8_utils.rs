use bevy_ecs_dynamic::reflect_value_ref::{EcsValueRef, ReflectValueRef};
use bevy_reflect_fns::ReflectFunction;
use deno_core::{error::AnyError, serde_v8, v8};
use std::mem::ManuallyDrop;

pub type ValueRefObject<'a> = serde_v8::Value<'a>;

pub unsafe fn reflect_value_ref_from_value<'a>(
    scope: &mut v8::HandleScope,
    value: ValueRefObject<'a>,
) -> Result<&'a ReflectValueRef, AnyError> {
    let transmit = reflect_value_ref_from_value_transmit(scope, value)?;
    transmit.value()
}
pub unsafe fn reflect_value_ref_from_value_transmit<'a>(
    scope: &mut v8::HandleScope,
    value: ValueRefObject<'a>,
) -> Result<&'a ReflectValueRefTransmit, AnyError> {
    reflect_value_ref_from_v8_value_transmit(scope, value.into())
}

pub unsafe fn reflect_value_ref_from_v8_value_transmit<'a>(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
) -> Result<&'a ReflectValueRefTransmit, AnyError> {
    let value: v8::Local<v8::Object> = value
        .try_into()
        .map_err(|e| anyhow::anyhow!("expected reflect value, got something {e}"))?;
    let external = value
        .get_internal_field(scope, 0)
        .ok_or_else(|| anyhow::anyhow!("expected reflect value, got something else"))?;
    let external: v8::Local<v8::External> = external
        .try_into()
        .map_err(|e| anyhow::anyhow!("expected reflect value, got something {e}"))?;

    let value = &*external.value().cast::<ReflectValueRefTransmit>();

    Ok(value)
}

pub unsafe fn extend_local_lifetime<'a, 'b, T>(val: v8::Local<'a, T>) -> v8::Local<'b, T> {
    std::mem::transmute(val)
}

pub enum ReflectValueRefTransmit {
    Value(ReflectValueRef),
    Method(ReflectValueRef, ReflectFunction),
}
impl ReflectValueRefTransmit {
    pub fn value(&self) -> Result<&ReflectValueRef, AnyError> {
        match self {
            ReflectValueRefTransmit::Value(value) => Ok(value),
            ReflectValueRefTransmit::Method(_, _) => Err(anyhow::anyhow!(
                "expected a reflect value, got a function reference"
            )),
        }
    }
    pub fn method(&self) -> Result<(&ReflectValueRef, &ReflectFunction), AnyError> {
        match self {
            ReflectValueRefTransmit::Method(value, method) => Ok((value, method)),
            ReflectValueRefTransmit::Value(_) => Err(anyhow::anyhow!(
                "expected a function reference, got a reflect value"
            )),
        }
    }
}
impl From<ReflectValueRef> for ReflectValueRefTransmit {
    fn from(value: ReflectValueRef) -> Self {
        ReflectValueRefTransmit::Value(value)
    }
}
impl From<EcsValueRef> for ReflectValueRefTransmit {
    fn from(value: EcsValueRef) -> Self {
        ReflectValueRefTransmit::Value(ReflectValueRef::from(value))
    }
}

pub unsafe fn create_value_ref_object(
    scope: &mut v8::HandleScope,
    value_ref: ReflectValueRefTransmit,
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
