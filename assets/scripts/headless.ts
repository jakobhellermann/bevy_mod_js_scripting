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
    let [translation1, translation2] = query.map((item) => item.components[0].translation);
    for (const s of [0.0, 0.25, 0.5, 0.75, 1.0]) {
      info(translation1.lerp(translation2, s).toString());
    }
  }
}

export default {
  update: run,
};
