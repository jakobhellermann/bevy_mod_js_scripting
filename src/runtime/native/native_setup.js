"use_strict";

((window) => {
    // Set the bevy scripting op function to Deno's opSync function
    window.bevyModJsScriptingOpSync = Deno.core.opSync;
})(globalThis);
