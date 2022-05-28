"use strict";

((window) => {
    window.trace = function (val) {
        Deno.core.opSync("op_log", "trace", val);
    }
    window.debug = function (val) {
        Deno.core.opSync("op_log", "debug", val)
    }
    window.info = function (val) {
        Deno.core.opSync("op_log", "info", val)
    }
    window.warn = function (val) {
        Deno.core.opSync("op_log", "warn", val)
    }
    window.error = function (val) {
        Deno.core.opSync("op_log", "error", val)
    }
})(globalThis);
