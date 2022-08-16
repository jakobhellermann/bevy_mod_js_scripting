use wasm_bindgen::prelude::*;

use super::{BevyModJsScripting, LOCK_SHOULD_NOT_FAIL};

use bevy::utils::tracing::{event, span, Level};

#[wasm_bindgen]
impl BevyModJsScripting {
    pub fn op_log(&self, level: &str, text: &str) {
        let state = self.state.try_lock().expect(LOCK_SHOULD_NOT_FAIL);
        let path = &state.current_script_path;

        let level: Level = level.parse().unwrap_or(Level::INFO);
        if level == Level::TRACE {
            let _span = span!(Level::TRACE, "script", ?path).entered();
            event!(target: "js_runtime", Level::TRACE, "{text}");
        } else if level == Level::DEBUG {
            let _span = span!(Level::DEBUG, "script", ?path).entered();
            event!(target: "js_runtime", Level::DEBUG, "{text}");
        } else if level == Level::INFO {
            let _span = span!(Level::INFO, "script", ?path).entered();
            event!(target: "js_runtime", Level::INFO, "{text}");
        } else if level == Level::WARN {
            let _span = span!(Level::WARN, "script", ?path).entered();
            event!(target: "js_runtime", Level::WARN, "{text}");
        } else if level == Level::ERROR {
            let _span = span!(Level::ERROR, "script", ?path).entered();
            event!(target: "js_runtime", Level::ERROR, "{text}");
        }
    }
}
