function filterComponentInfos(
  infos: ComponentInfo[],
  prefix: string
): string[] {
  return infos
    .filter((info) => info.name.startsWith(prefix))
    .map((info) => info.name.replace(prefix, ""));
}

function componentId(name) {
  let id = world.components.find((info) => info.name === name);
  if (!id) throw new Error(`component id for ${name} not found`);
  return id.id;
}

let firstIteration = true;

export default {
  update() {
    if (firstIteration) {
      firstIteration = false;

      for (const entity of world.entities) {
        info(entity);
      }
    }
  },
};
