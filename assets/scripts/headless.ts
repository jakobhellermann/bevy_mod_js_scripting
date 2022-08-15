let i = 0;

type Scoreboard = {
  score: number;
  extra: ExtraData;
};
type ExtraData = {
  name: string;
};
const Scoreboard: BevyType<Scoreboard> = { typeName: "headless::Scoreboard" };

info("Loaded");

export default {
  update() {
    if (i == 0) {
      info(world.resources);
    }

    if (i % 3 == 0) {
      let score = world.resource(Scoreboard);
      info(score.toString());
      info(score.score);
      info(score.extra.name);
    }

    i++;
  },
};
