use std::path::PathBuf;

use bevy::{prelude::*, utils::HashMap};
use type_map::TypeMap;

use crate::asset::JsScript;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

pub mod ops;

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

    /// Run during an exclusive system executed before [`CoreStage::First`] to allow the engine to
    /// do any pre-frame preparation.
    fn frame_start(&self, world: &mut World) {
        let _ = world;
    }

    /// Run during an exclusive system executed after [`CoreStage::Last`] to allow the engine to
    /// do any post-frame operations.
    fn frame_end(&self, world: &mut World) {
        let _ = world;
    }
}

// Hash map of op names to op implementation
pub type OpMap = HashMap<&'static str, Box<dyn JsRuntimeOp>>;

/// Contains mapping from op index to op name
#[derive(Deref, DerefMut, Debug)]
struct OpNames(HashMap<usize, &'static str>);

/// Contains mapping from op name to op index
pub type OpIndexes = HashMap<&'static str, usize>;

/// List of [`JsRuntimeOp`]s installed for the [`JsRuntime`]
pub type Ops = Vec<Box<dyn JsRuntimeOp>>;

struct InvalidOp;
impl JsRuntimeOp for InvalidOp {
    fn run(
        &self,
        _context: OpContext,
        _world: &mut World,
        _args: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        anyhow::bail!(
            "Invalid operation. You may have forgotten to register a custom JsRuntimeOp."
        );
    }
}

fn get_ops(custom_ops: OpMap) -> (Ops, OpIndexes, OpNames) {
    // Collect core ops
    let mut op_map = ops::get_core_ops();

    // Add custom ops to core ops and warn about conflicts
    for (op_name, op) in custom_ops.into_iter() {
        if op_map.insert(op_name, op).is_some() {
            warn!(
                "Custom op name {op_name} conflicts with core op with \
                    the same name. Custom op will take precedence."
            );
        }
    }

    // Collect ops into a vector while mapping the op names to it's index in the vector
    let mut ops: Ops = Vec::with_capacity(op_map.len() + 1);
    ops.push(Box::new(InvalidOp)); // The first op is the invalid op called when an op is not found
    let op_indexes = op_map
        .into_iter()
        .map(|(name, op)| {
            ops.push(op);
            (name, ops.len() - 1)
        })
        .collect::<HashMap<_, _>>();

    // Create reverse lookup so we can get the op name from the index for debugging/logging purposes
    let op_names = op_indexes
        .clone()
        .into_iter()
        .map(|(k, v)| (v, k))
        .collect();
    let op_names = OpNames(op_names);

    (ops, op_indexes, op_names)
}

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
    pub handle: Handle<JsScript>,
}

pub struct OpContext<'a> {
    pub op_state: &'a mut TypeMap,
    pub script_info: &'a ScriptInfo,
}
pub trait JsRuntimeOp {
    /// Returns any extra JavaScript that should be executed when the runtime is initialized.
    fn js(&self) -> Option<&'static str> {
        None
    }

    /// The function called to execute the operation
    fn run(
        &self,
        context: OpContext<'_>,
        world: &mut World,
        args: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        // Satisfy linter without changing argument names for the sake of the API docs
        let (_, _, _) = (context, world, args);

        // Ops may be inserted simply to add JS, so a default implementation of `run` is useful to
        // indicate that the op is not meant to be run.
        anyhow::bail!("Op is not meant to be called");
    }

    /// Function called at  to allow the op to do any preparation work
    fn frame_start(&self, op_state: &mut TypeMap, world: &mut World) {
        // Fix clippy warning by using variables
        let _ = (op_state, world);
    }

    fn frame_end(&self, op_state: &mut TypeMap, world: &mut World) {
        // Fix clippy warning by using variables
        let _ = (op_state, world);
    }
}

impl<T: Fn(OpContext<'_>, &mut World, serde_json::Value) -> anyhow::Result<serde_json::Value>>
    JsRuntimeOp for T
{
    fn run(
        &self,
        context: OpContext<'_>,
        world: &mut World,
        args: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        self(context, world, args)
    }
}
