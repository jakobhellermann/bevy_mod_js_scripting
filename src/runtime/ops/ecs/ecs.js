"use strict";

((window) => {
    class ComponentId {
        index;
    }

    class Entity {
        #bits;
        id;
        generation;
    }

    class QueryItems extends Array {
        get(entity) {
            const r = this.filter(x => x.entity.eq(entity))[0];
            return r && r.components;
        }
    }

    class World {
        toString() {
            return bevyModJsScriptingOpSync("ecs_world_to_string", this.rid);
        }

        get components() {
            return bevyModJsScriptingOpSync("ecs_world_components", this.rid);
        }

        get resources() {
            return bevyModJsScriptingOpSync("ecs_world_resources", this.rid);
        }

        get entities() {
            return bevyModJsScriptingOpSync("ecs_world_entities", this.rid);
        }

        resource(componentId) {
            let resource = bevyModJsScriptingOpSync(
                "ecs_world_get_resource",
                componentId
            );
            return resource != null ? Value.wrapValueRef(resource) : null;
        }

        query(...parameters) {
            // Helper to collect and cache query results in the target
            const collectedQuery = (target) => {
                if (target.collected) {
                    return target.collected;
                } else {
                    target.collected = QueryItems.from(bevyModJsScriptingOpSync(
                        "ecs_world_query_collect",
                        parameters,
                    ).map(({ entity, components }) => ({
                        entity: Value.wrapValueRef(entity),
                        components: components.map(Value.wrapValueRef),
                    })));

                    return target.collected;
                }
            };

            const target = { parameters, collected: null };
            return new Proxy(target, {
                get(target, propName) {
                    switch (propName) {
                        // Optimize the special case of accessing the components of a single entity.
                        case "get":
                            return (entity) => {
                                let ret = bevyModJsScriptingOpSync(
                                    "ecs_world_query_get",
                                    Value.unwrapValueRef(entity),
                                    target.parameters
                                );
                                return ret ? ret.map(Value.wrapValueRef) : undefined;
                            };
                        // Default to collecting all the query results and returning the array prop.
                        default:
                            const collected = collectedQuery(target);
                            const prop = collected[propName];
                            return prop.bind ? prop.bind(collected) : prop;
                    }
                }
            })
        }

        get(entity, component) {
            const r = bevyModJsScriptingOpSync("ecs_world_query_get", Value.unwrapValueRef(entity), [component]);
            return r[0] && Value.wrapValueRef(r[0]);
        }

        insert(entity, component) {
            bevyModJsScriptingOpSync(
                "ecs_component_insert",
                Value.unwrapValueRef(entity),
                Value.unwrapValueRef(component)
            );
        }

        spawn() {
            return Value.wrapValueRef(bevyModJsScriptingOpSync("ecs_entity_spawn"));
        }
    }

    const VALUE_REF_GET_INNER = Symbol("value_ref_get_inner");

    globalThis.Value = {
        // tries to unwrap the inner value ref, otherwise returns the value unchanged
        unwrapValueRef(valueRefProxy) {
            if (valueRefProxy === null || valueRefProxy === undefined) return valueRefProxy;
            const inner = valueRefProxy[VALUE_REF_GET_INNER]
            if (inner) {
                return inner;
            } else {
                if (typeof valueRefProxy == 'object') {
                    for (const key of Reflect.ownKeys(valueRefProxy)) {
                        valueRefProxy[key] = Value.unwrapValueRef(valueRefProxy[key]);
                    }
                }
                return valueRefProxy;
            }
        },

        // keep primitives, null and undefined as is, otherwise wraps the object
        // in a proxy
        wrapValueRef(valueRef) {
            // leaf primitives
            if (typeof valueRef !== "object" || valueRef === null || valueRef === undefined) {
                return valueRef;
            }

            let target = () => { };
            target.valueRef = valueRef;
            const proxy = new Proxy(target, {
                ownKeys: (target) => {
                    return [
                        ...bevyModJsScriptingOpSync(
                            "ecs_value_ref_keys",
                            target.valueRef
                        ),
                        VALUE_REF_GET_INNER,
                    ];
                },
                get: (target, p, receiver) => {
                    switch (p) {
                        case VALUE_REF_GET_INNER:
                            return target.valueRef;
                        case "toString":
                            return () =>
                                bevyModJsScriptingOpSync(
                                    "ecs_value_ref_to_string",
                                    target.valueRef
                                );
                        case "eq":
                            return (otherRef) =>
                                bevyModJsScriptingOpSync(
                                    "ecs_value_ref_eq",
                                    target.valueRef,
                                    Value.unwrapValueRef(otherRef),
                                );
                        default:
                            let valueRef = bevyModJsScriptingOpSync(
                                "ecs_value_ref_get",
                                target.valueRef,
                                p,
                            );
                            return Value.wrapValueRef(valueRef);
                    }
                },
                set: (target, p, value) => {
                    bevyModJsScriptingOpSync(
                        "ecs_value_ref_set",
                        target.valueRef,
                        p,
                        Value.unwrapValueRef(value)
                    );
                },
                apply: (target, thisArg, args) => {
                    let ret = bevyModJsScriptingOpSync(
                        "ecs_value_ref_call",
                        target.valueRef,
                        args.map((arg) => {
                            return Value.unwrapValueRef(arg);
                        })
                    );
                    return Value.wrapValueRef(ret);
                },
            });
            return proxy;
        },

        // Instantiates the default value of a given bevy type
        create(type, patch) {
            return Value.wrapValueRef(bevyModJsScriptingOpSync("ecs_value_ref_default", type.typeName, Value.unwrapValueRef(patch)));
        },

        patch(value, patch) {
            Value.wrapValueRef(bevyModJsScriptingOpSync("ecs_value_ref_patch", Value.unwrapValueRef(value), patch));
        }
    }

    const world = new World();
    window.world = world;
})(globalThis);
