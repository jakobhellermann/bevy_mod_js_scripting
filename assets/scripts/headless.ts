let firstIteration = true;

export default {
  update() {
    if (firstIteration) {
      firstIteration = false;

      info("Components: " + world.components.map(info => info.name).join(", "));
      info("Resources:");
      info(world.resources.map(info => info.name));
      info("Resources (headless): " + filterComponentInfos(world.resources, "headless::").join(", "));
      info("Entitites: " + (world.entities.map(entity => `Entity(${entity.id}v${entity.generation})`).join(", ")));

      info("----------");

      let transformId = componentId(
        "bevy_transform::components::transform::Transform"
      );

      let query = world.query({
        components: [transformId],
      });
      let [transform1, transform2] = query.map((item) => item.components[0]);
      let [translation1, translation2] = [
        transform1.translation,
        transform2.translation,
      ];

      for (const s of [0.0, 0.25, 0.5, 0.75, 1.0]) {
        info(translation1.lerp(translation2, s).toString());
      }
    }
  },
};

function componentId(name: string) {
  let id = world.components.find((info) => info.name === name);
  if (!id) throw new Error(`component id for ${name} not found`);
  return id.id;
}

function filterComponentInfos(
  infos: ComponentInfo[],
  prefix: string
): string[] {
  return infos
    .filter((info) => info.name.startsWith(prefix))
    .map((info) => info.name.replace(prefix, ""));
}
