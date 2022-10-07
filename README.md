# bevy_mod_js_scripting

`bevy_mod_js_scripting` is an experimental scripting integration crate for writing javascript/typescript scripts and running them
in `bevy` with full access to the ECS values like components and resources.


## Example

```ts
// type safe access to resources and values
type Scoreboard = { score: number };
const Scoreboard: BevyType<Scoreboard> = { typeName: "breakout::Scoreboard" };

// script-local variables can be used for easy cross-frame state
let i = 0;

function run() {
    // increment score every 60 frames
    if(i % 60 == 0) {
        let score = world.resource(Scoreboard)!;
        score.score += 1;
        // logging works via `trace`, `debug`, `info`, `warn`, `error`
        info(score.score);
    }

    // query components
    for (const item of world.query(Transform, Aabb)) {
        let [transform, aabb] = item.components;
        info("Translation:", transform.translation.toString());
        info("AABB Center:", aabb.center.toString());

        // call methods on value references (requires app code setup, see headless.rs)
        let normalized = transform.scale.normalize();
    }
}

export default {
    // execute the `run` function in the update stage
    update: run,
}
```

More examples can be found in the [examples](./examples/) folder.
Also check out the [punchy wiki page](https://github.com/fishfolks/punchy/wiki/Scripting) on scripting, which uses `bevy_mod_js_scripting`.

## Current Status

Currently supported operations are 
- resource access (`world.resource(Time)`)
- world information (`world.components`, `world.resources`, `world.entities`)
- queries (`world.query(Ball, Velocity).map(({ entity, components }) => components[1])`)
- component insertion (`world.insert(value)`)
- dealing with ecs value references (`Value.create`, `Value.patch`)

## Design decisions

<details>
<summary>Types</summary>

In `bevy_ecs`, the common methods for dealing with ECS values all take a type parameter, like 
```rs
world.resource::<T>(); // or
world.query<(Entity, &Component)>();
```
Ideally we would be able to write
```ts
let time = world.resource<Time>();
```
as well in typescript, but since typescript just transpiles to javascript without adding any new runtime capabilities, we cannot associate any runtime values with the `Time` type.


Instead, what we need to do is write `type` definition with an associated variable of type `BevyType<T>`, which contains the referenced type's type name.

```ts
type Transform = {
  translation: Vec3,
  rotation: Quat,
  scale: Vec3,
};
const Transform: BevyType<Transform> = {
    typeName: "bevy_transform::components::global_transform::GlobalTransform"
};

// `world.resource` is typed so that typescript can infer `transform` to be of type `Transform | undefined`
let transform = world.resource(Transform);
```

Similarly, queries list their types like
```ts
for (item of world.query(Ball, Velocity)) {
    info(item.entity);
    let [_ball, velocity] = item.components;
    // velocity is properly typed
}
```

Currently, there is a pregenerated list of bevy types in [./types/bevy_types.ts](./types/bevy_types.ts), and you can also just define your own ones.
    
In the future we may include a utility for automatically generating the typescript definitions for your game in a `build.rs` script, so that you don't need to manually write or re-generate them.

</details>
<details>
<summary>Javascript Values & ECS Value references</summary>

When you call `world.resource` (or any other method returning references to ECS values), what you get is not just a simple javascript object corresponding to the rust value, but instead a `Proxy` which defers all accesses/modifications to the actual value inside the bevy world.

Only leaf values, like `transform.translation.x`, which can be natively represented as a javascript primitive, are automatically converted to/from the rust representation on gets and sets.

This means that
```ts
let transform = world.resource(Transform);
let translation = transform.translation;
// typeof translation.x == "number"
translation.x = 3.0;
```

If you want to create a new value reference, for example for inserting a new resource, the current APIs to do that are `Value.create` and `Value.patch`.

```ts
let transform = Value.create(Transform);
let vec3 = Value.create(Vec3, { x: 0.0, y: 1.0, z: 2.0 });
transform.translation = vec3;
world.insertResource(Transform, transform);
```

Expect to see changes in this area as we figure out the best way to deal with the interaction of javascript objects and value references.
</details>

## Web support

`bevy_mod_js_scripting` can run in the browser using its native javascript execution environment.
To try it out, download and configure [wasm-server-runner](https://github.com/jakobhellermann/wasm-server-runner) and run
```sh
cargo run --example breakout --target wasm32-unknown-unknown
```
