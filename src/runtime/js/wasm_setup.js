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
        case "op_world_query":
            return window.bevyModJsScripting.op_world_query(WORLD_RID, args[1]);
        case "op_world_get_resource":
            return window.bevyModJsScripting.op_world_get_resource(WORLD_RID, args[1]);
        case "op_value_ref_get":
            return window.bevyModJsScripting.op_value_ref_get(WORLD_RID, args[1], args[2]);
        case "op_value_ref_set":
            return window.bevyModJsScripting.op_value_ref_set(WORLD_RID, args[1], args[2], args[3]);
        case "op_value_ref_keys":
            return window.bevyModJsScripting.op_value_ref_keys(WORLD_RID, args[1]);
        case "op_value_ref_call":
            return window.bevyModJsScripting.op_value_ref_call(WORLD_RID, args[1], args[2]);
        case "op_value_ref_to_string":
            return window.bevyModJsScripting.op_value_ref_to_string(WORLD_RID, args[1]);
        default:
            console.error(`Op not implemented for browser yet: ${op_name}`);
            return;
    }
}
