let i = 0;
function run() {
    i += 1;
    if (i % 60 == 0) {
        info(world.toString());
        warn(world.toString());
        trace(world.toString());
        error(world.toString());
    }
}
