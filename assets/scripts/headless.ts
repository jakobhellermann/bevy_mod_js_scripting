let firstIteration = true;
function run() {
  if (firstIteration) {
    firstIteration = false;
    // info("Components: " + world.components.map(info => info.name).join(", "));
    // info("Resources:");
    // info(world.resources.map(info => info.name));
    // info("Resources (headless): " + filterComponentInfos(world.resources, "headless::").join(", "));
    // info("Entitites: " + (world.entities.map(entity => `Entity(${entity.id}v${entity.generation})`).join(", ")));

    let query = world.query(Transform);
    for (const item of query) {
      let [transform] = item.components;
      let target = Value.create(Vec3, { x: 100, y: 100, z: 100 });
      info(transform.translation.lerp(target, 0.5).toString());
    }
  }
}

export default {
  update: run,
};
