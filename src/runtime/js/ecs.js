"use strict";

((window) => {
    class World {
        get #rid() { return 0 }

        toString() {
            return Deno.core.opSync("op_world_tostring", this.rid);
        }
    }

    const world = new World();
    window.world = world;
})(globalThis);
