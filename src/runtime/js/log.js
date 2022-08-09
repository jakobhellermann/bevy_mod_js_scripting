"use strict";

((window) => {
    window.trace = function (val) {
        bevyModJsScriptingOpSync("op_log", "trace", val);
    };
    window.debug = function (val) {
        bevyModJsScriptingOpSync("op_log", "debug", val);
    };
    window.info = function (val) {
        bevyModJsScriptingOpSync("op_log", "info", val);
    };
    window.warn = function (val) {
        bevyModJsScriptingOpSync("op_log", "warn", val);
    };
    window.error = function (val) {
        bevyModJsScriptingOpSync("op_log", "error", val);
    };
})(globalThis);
