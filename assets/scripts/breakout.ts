
function filterComponentInfos(infos: ComponentInfo[], prefix: string): string[] {
    return infos
        .filter(info => info.name.startsWith(prefix))
        .map(info => info.name.replace(prefix, ""));
}


let firstIteration = true;
let i = 0;

type Scoreboard = {
    score: number;
};
const Scoreboard: BevyType<Scoreboard> = { typeName: "breakout::Scoreboard" };

function run() {
    i++;
    if (i % 60 == 0) {
        let time = world.resource(Scoreboard);
        time.score += 1;
        info(time.score);
    }

    if (firstIteration) {
        firstIteration = false;
        // info("Components: " + filterComponentInfos(world.components, "bevy_transform::"));
        // info("Resources: " + filterComponentInfos(world.resources, "breakout::").join(", "));
    }
}
