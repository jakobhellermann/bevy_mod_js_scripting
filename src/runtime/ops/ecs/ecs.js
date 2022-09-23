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
            return resource != null ? wrapValueRef(resource) : null;
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
                        entity: wrapValueRef(entity),
                        components: components.map(wrapValueRef),
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
                            return (entity) => bevyModJsScriptingOpSync(
                                "ecs_world_query_get",
                                entity[VALUE_REF_GET_INNER].valueRef,
                                target.parameters
                            );
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
            const r = bevyModJsScriptingOpSync("ecs_world_query_get", entity, component);
            return r[0] && wrapValueRef(r[0]);
        }
    }

    globalThis.VALUE_REF_GET_INNER = Symbol("value_ref_get_inner");
    const valueRefFinalizationRegistry = new FinalizationRegistry(ref => {
        bevyModJsScriptingOpSync("ecs_value_ref_free", ref);
    });
    globalThis.wrapValueRef = (valueRef) => {
        // leaf primitives
        if (typeof valueRef !== "object") {
            return valueRef;
        }

        const refCopy = { key: valueRef.key, function: valueRef.function };
        valueRefFinalizationRegistry.register(valueRef, refCopy);

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
                        return target;
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
                                otherRef[VALUE_REF_GET_INNER].valueRef
                            );
                    default:
                        let valueRef = bevyModJsScriptingOpSync(
                            "ecs_value_ref_get",
                            target.valueRef,
                            p,
                        );
                        return wrapValueRef(valueRef);
                }
            },
            set: (target, p, value) => {
                bevyModJsScriptingOpSync(
                    "ecs_value_ref_set",
                    target.valueRef,
                    p,
                    value
                );
            },
            apply: (target, thisArg, args) => {
                let ret = bevyModJsScriptingOpSync(
                    "ecs_value_ref_call",
                    target.valueRef,
                    args.map((arg) => {
                        let valueRef = arg[VALUE_REF_GET_INNER]?.valueRef;
                        return valueRef !== undefined ? valueRef : arg;
                    })
                );
                return wrapValueRef(ret);
            },
        });
        return proxy;
    }

    const world = new World();
    window.world = world;
})(globalThis);
