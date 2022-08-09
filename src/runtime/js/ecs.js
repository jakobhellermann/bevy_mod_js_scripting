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

    class World {
        get #rid() { return 0 }

        toString() {
            return Deno.core.opSync("op_world_tostring", this.rid);
        }

        get components() {
            return Deno.core.opSync("op_world_components", this.rid);
        }

        get resources() {
            return Deno.core.opSync("op_world_resources", this.rid);
        }

        get entities() {
            return Deno.core.opSync("op_world_entities", this.rid);
        }

        query(descriptor) {
            return Deno.core.opSync("op_world_query", this.rid, descriptor)
                .map(({ entity, components }) => ({
                    entity,
                    components: components.map(wrapValueRef),
                }));
        }
    }


    const VALUE_REF_GET_INNER = Symbol("value_ref_get_inner");
    function wrapValueRef(valueRef) {
        // leaf primitives
        if (typeof valueRef !== "object") { return valueRef };
        let target = () => { };
        target.valueRef = valueRef;
        const proxy = new Proxy(target, {
            ownKeys: (target) => {
                return [...Deno.core.opSync("op_value_ref_keys", world.rid, target.valueRef), VALUE_REF_GET_INNER];
            },
            get: (target, p, receiver) => {
                switch (p) {
                    case VALUE_REF_GET_INNER:
                        return target;
                    case "toString":
                        return () => Deno.core.opSync("op_value_ref_to_string", world.rid, target.valueRef);
                    default:
                        let valueRef = Deno.core.opSync("op_value_ref_get", world.rid, target.valueRef, "." + p);
                        return wrapValueRef(valueRef);
                }
            },
            set: (target, p, value) => {
                Deno.core.opSync("op_value_ref_set", world.rid, target.valueRef, "." + p, value);
            },
            apply: (target, thisArg, args) => {
                return Deno.core.opSync("op_value_ref_call", world.rid, target.valueRef, args.map(arg => {
                    let valueRef = arg[VALUE_REF_GET_INNER]?.valueRef;
                    return (valueRef !== undefined) ? valueRef : arg;
                }));
            }
        });
        return proxy;
    }

    const world = new World();
    window.world = world;
})(globalThis);
