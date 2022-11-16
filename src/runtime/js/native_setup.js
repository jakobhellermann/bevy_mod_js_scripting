"use strict";

((window) => {
    // The placeholder below is replaced with an object mapping op names to op indexes
    const OP_NAME_MAP = __OP_NAME_MAP_PLACEHOLDER__;

    // Set the bevy scripting op function to Deno's opSync function
    window.bevyModJsScriptingOpSync = (op_name, ...args) => {
        try {
            return Deno.core.opSync("op_bevy_mod_js_scripting", OP_NAME_MAP[op_name], args);
        } catch (e) {
            throw `Error during \`${op_name}\`: ${e}`
        }
    }
})(globalThis);
