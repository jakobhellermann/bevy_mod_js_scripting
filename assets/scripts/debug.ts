
function filterComponentInfos(infos: ComponentInfo[], prefix: string): string[] {
    return infos
        .filter(info => info.name.startsWith(prefix))
        .map(info => info.name.replace(prefix, ""));
}

let firstIteration = true;
function run() {
    if (firstIteration) {
        firstIteration = false;

        info("Components: " + filterComponentInfos(world.components, "breakout::"));
        info("Resources: " + filterComponentInfos(world.resources, "breakout::").join(", "));

        let ballId = world.components.find(info => info.name == "breakout::Ball").id;
        let velocityId = world.components.find(info => info.name == "breakout::Velocity").id;

        const ballQuery = world.query({
            components: [ballId, velocityId],
        });
        for (const item of ballQuery) {
            let [ball, velocity] = item.components;
            info(item.entity);
            info(velocity);
        }
    }
}
