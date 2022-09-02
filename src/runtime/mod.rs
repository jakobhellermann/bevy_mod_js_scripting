use bevy::prelude::*;

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
