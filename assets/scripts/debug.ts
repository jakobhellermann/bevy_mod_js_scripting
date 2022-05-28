
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
    }
}
