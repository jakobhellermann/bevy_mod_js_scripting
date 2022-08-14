export function setup_js_globals(bevyModJsScripting) {
    window.bevyModJsScripting = bevyModJsScripting;
    window.bevyModJsScriptingOpSync = bevyModJsScriptingOpSync;
}

function bevyModJsScriptingOpSync(op_name, ...args) {
    const WORLD_RID = 0;
    switch (op_name) {
        case "op_log":
            return window.bevyModJsScripting.op_log(
                args[0],
                JSON.stringify(args[1])
            );
        case "op_world_tostring":
            return window.bevyModJsScripting.op_world_tostring(WORLD_RID);
        case "op_world_components":
            return window.bevyModJsScripting.op_world_components(WORLD_RID);
        case "op_world_resources":
            return window.bevyModJsScripting.op_world_resources(WORLD_RID);
        case "op_world_entities":
            return window.bevyModJsScripting.op_world_entities(WORLD_RID);
        default:
            console.error(`Op not implemented for browser yet: ${op_name}`);
            return;
    }
}
