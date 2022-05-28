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
    }

    const world = new World();
    window.world = world;
})(globalThis);
