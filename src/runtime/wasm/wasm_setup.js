export function setup_js_globals(bevyModJsScripting) {
    window.bevyModJsScripting = bevyModJsScripting;
    window.bevyModJsScriptingOpSync = bevyModJsScriptingOpSync;
}

function bevyModJsScriptingOpSync(op_name, ...args) {
    switch (op_name) {
        case "op_log":
            window.bevyModJsScripting.op_log(args[0], JSON.stringify(args[1]));
    }
}
