declare namespace Deno {
    namespace core {
        function opSync(op: string, ...args: any[]): any;

    }
}

// log.s
declare function trace(val: any): void;
declare function debug(val: any): void;
declare function info(val: any): void;
declare function warn(val: any): void;
declare function error(val: any): void;

// ecs.js
declare class ComponentId {
    index: number;
}
declare class Entity {
    id: number;
    generation: number;
}

type ComponentInfo = {
    id: ComponentId,
    name: string,
    size: number;
};

type QueryDescriptor = {
    components: ComponentId[],
};

type QueryItem = {
    entity: Entity,
    components: any[],
};

type Primitive = number | string | boolean;
interface Value {
    [path: string | number]: Value | Primitive | undefined,
}

declare class World {
    get components(): ComponentInfo[];
    get resources(): ComponentInfo[];
    get entities(): Entity[];

    resource(componentId: ComponentId): Value | null;

    query(descriptor: QueryDescriptor): QueryItem[];
}

declare let world: World;
