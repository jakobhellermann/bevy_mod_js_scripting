function filterComponentInfos(infos: ComponentInfo[], prefix: string): string[] {
    return infos
        .filter(info => info.name.startsWith(prefix))
        .map(info => info.name.replace(prefix, ""));
}

let firstIteration = true;
function run() {
    if (firstIteration) {
        firstIteration = false;

        info("Components: " + world.components.map(info => info.name).join(", "));
        info("Resources:");
        info(world.resources.map(info => info.name));
        info("Resources (headless): " + filterComponentInfos(world.resources, "headless::").join(", "));
        info("Entitites: " + (world.entities.map(entity => `Entity(${entity.id}v${entity.generation})`).join(", ")));
    }
}
