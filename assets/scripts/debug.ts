
function filterComponentInfos(infos: ComponentInfo[], prefix: string): string[] {
    return infos
        .filter(info => info.name.startsWith(prefix))
        .map(info => info.name.replace(prefix, ""));
}


let firstIteration = true;
let i = 0;
function run() {
    if (firstIteration) {
        firstIteration = false;

        info("Components: " + filterComponentInfos(world.components, "bevy_transform::"));
        info("Resources: " + filterComponentInfos(world.resources, "breakout::").join(", "));
    }


    i++;
    if (i % 60 == 0) {
        let ballId = world.components.find(info => info.name == "breakout::Ball").id;
        let velocityId = world.components.find(info => info.name == "breakout::Velocity").id;
        let transformId = world.components.find(info => info.name == "bevy_transform::components::transform::Transform").id;

        const ballQuery = world.query({
            components: [ballId, transformId, velocityId],
        });
        for (const item of ballQuery) {
            let [ball, transform, velocity] = item.components;
            velocity = velocity[0];

            info(velocity.toString());
        }
    }
}
