use std::{path::PathBuf, sync::Arc};

use bevy::{prelude::*, utils::HashMap};

use crate::asset::JsScript;

mod types;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

mod ops;

/// The API implemented by different script runtimes.
///
/// Currently we have a native runtime built on [`deno_core`] and a web runtime utilizing
/// [`wasm_bindgen`].
pub trait JsRuntimeApi: FromWorld {
    /// Load a script
    ///
    /// This will not reload a script that has already been loaded unless `reload` is set to `true`.
    fn load_script(&self, handle: &Handle<JsScript>, script: &JsScript, reload: bool);

    /// Returns whether or not a script has been loaded yet
    fn has_loaded(&self, handle: &Handle<JsScript>) -> bool;

    /// Run a script
    fn run_script(&self, handle: &Handle<JsScript>, stage: &CoreStage, world: &mut World);
}

pub type OpMap = HashMap<&'static str, Arc<dyn JsRuntimeOp>>;

/// Resource that may be inserted before adding the [`JsScriptingPlugin`][crate::JsScriptingPlugin]
/// to configure the JS runtime.
#[derive(Default)]
pub struct JsRuntimeConfig {
    /// Mapping of custom operations that may be called from the JavaScript environment.
    ///
    /// The string key is the op name which must be passed as the first argument of the
    /// `bevyModJsScriptingOpSync` JS global when executing the op.
    pub custom_ops: OpMap,
}

/// Info about the currently executing script, exposed to [`JsRuntimeOp`]s.
pub struct ScriptInfo {
    pub path: PathBuf,
}

pub trait JsRuntimeOp {
    /// Returns any extra JavaScript that should be executed when the runtime is initialized.
    fn js(&self) -> Option<&'static str> {
        None
    }

    /// The function called to execute the operation
    fn run(
        &self,
        script_info: &ScriptInfo,
        world: &mut World,
        args: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        // Satisfy linter without changing argument names for the sake of the API docs
        let (_, _, _) = (script_info, world, args);

        // Ops may be inserted simply to add JS, so a default implementation of `run` is useful to
        // indicate that the op is not meant to be run.
        anyhow::bail!("Op is not meant to be called");
    }
}

impl<T: Fn(&ScriptInfo, &mut World, serde_json::Value) -> anyhow::Result<serde_json::Value>>
    JsRuntimeOp for T
{
    fn run(
        &self,
        script_info: &ScriptInfo,
        world: &mut World,
        args: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        self(script_info, world, args)
    }
}
