use deno_core::{JsRuntime, RuntimeOptions};

pub fn create_runtime() -> JsRuntime {
    let runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![],
        ..Default::default()
    });
    runtime
}
