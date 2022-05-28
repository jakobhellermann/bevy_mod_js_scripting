// log.s
declare function trace(val: any);
declare function debug(val: any);
declare function info(val: any);
declare function warn(val: any);
declare function error(val: any);

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
    components: unknown[],
};

declare class World {
    get components(): ComponentInfo[];
    get resources(): ComponentInfo[];
    get entities(): Entity[];

    query(descriptor: QueryDescriptor): QueryItem[];
}

declare let world: World;
