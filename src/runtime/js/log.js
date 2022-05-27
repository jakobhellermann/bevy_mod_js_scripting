"use strict";

((window) => {
    window.trace = function (val) {
        Deno.core.opSync("op_log", "trace", val.toString())
    }
    window.debug = function (val) {
        Deno.core.opSync("op_log", "debug", val.toString())
    }
    window.info = function (val) {
        Deno.core.opSync("op_log", "info", val.toString())
    }
    window.warn = function (val) {
        Deno.core.opSync("op_log", "warn", val.toString())
    }
    window.error = function (val) {
        Deno.core.opSync("op_log", "error", val.toString())
    }
})(globalThis);
