use anyhow::Context;
use bevy::{
    log::Level,
    prelude::*,
    utils::tracing::{event, span},
};
use serde::Deserialize;
use type_map::TypeMap;

use crate::runtime::{JsRuntimeOp, ScriptInfo};

pub struct OpLog;

#[derive(Deserialize)]
struct OpLogArgs {
    level: u8,
    args: Vec<serde_json::Value>,
}

impl JsRuntimeOp for OpLog {
    fn run(
        &self,
        _op_state: &mut TypeMap,
        script_info: &ScriptInfo,
        _world: &mut World,
        args: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let args: OpLogArgs = serde_json::from_value(args).context("Parse `log` op args")?;
        let level = args.level;

        let text = args
            .args
            .iter()
            .map(|arg| {
                // Print string args without quotes
                if let serde_json::Value::String(s) = &arg {
                    s.clone()
                // Format other values as JSON
                } else {
                    format!("{arg}")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        match level {
            0 => {
                let _span = span!(Level::TRACE, "script", path = ?script_info.path).entered();
                event!(target: "js_runtime", Level::TRACE, "{text}");
            }
            1 => {
                let _span = span!(Level::DEBUG, "script", path = ?script_info.path).entered();
                event!(target: "js_runtime", Level::DEBUG, "{text}");
            }
            2 => {
                let _span = span!(Level::INFO, "script", path = ?script_info.path).entered();
                event!(target: "js_runtime", Level::INFO, "{text}");
            }
            3 => {
                let _span = span!(Level::WARN, "script", path = ?script_info.path).entered();
                event!(target: "js_runtime", Level::WARN, "{text}");
            }
            4 => {
                let _span = span!(Level::ERROR, "script", path = ?script_info.path).entered();
                event!(target: "js_runtime", Level::ERROR, "{text}");
            }
            _ => anyhow::bail!("Invalid log level"),
        }

        Ok(serde_json::Value::Null)
    }

    fn js(&self) -> Option<&'static str> {
        Some(
            r#"
            globalThis.trace = (...args) => bevyModJsScriptingOpSync("log", 0, args);
            globalThis.debug = (...args) => bevyModJsScriptingOpSync("log", 1, args);
            globalThis.info = (...args) => bevyModJsScriptingOpSync("log", 2, args);
            globalThis.warn = (...args) => bevyModJsScriptingOpSync("log", 3, args);
            globalThis.error = (...args) => bevyModJsScriptingOpSync("log", 4, args);
            "#,
        )
    }
}
