use bevy::utils::tracing::{event, span, Level};
use deno_core::{error::AnyError, include_js_files, op, Extension, OpState};

use super::ScriptInfo;

#[op]
fn op_log(state: &mut OpState, kind: String, text: serde_json::Value) -> Result<(), AnyError> {
    let info = state.borrow::<ScriptInfo>();

    let text = serde_json::to_string_pretty(&text)?;

    let level: Level = kind.parse()?;
    if level == Level::TRACE {
        let _span = span!(Level::TRACE, "script", path = ?info.path).entered();
        event!(target: "js_runtime", Level::TRACE, "{text}");
    } else if level == Level::DEBUG {
        let _span = span!(Level::DEBUG, "script", path = ?info.path).entered();
        event!(target: "js_runtime", Level::DEBUG, "{text}");
    } else if level == Level::INFO {
        let _span = span!(Level::INFO, "script", path = ?info.path).entered();
        event!(target: "js_runtime", Level::INFO, "{text}");
    } else if level == Level::WARN {
        let _span = span!(Level::WARN, "script", path = ?info.path).entered();
        event!(target: "js_runtime", Level::WARN, "{text}");
    } else if level == Level::ERROR {
        let _span = span!(Level::ERROR, "script", path = ?info.path).entered();
        event!(target: "js_runtime", Level::ERROR, "{text}");
    } else {
        return Err(anyhow::anyhow!("unknown log level {level:?}"));
    }

    Ok(())
}

pub fn extension() -> Extension {
    Extension::builder()
        .ops(vec![op_log::decl()])
        .js(include_js_files!(prefix "bevy", "js/log.js",))
        .build()
}
