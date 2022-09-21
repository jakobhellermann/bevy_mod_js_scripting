let firstIteration = true;
let i = 0;

type Scoreboard = {
  score: number;
};
const Scoreboard: BevyType<Scoreboard> = { typeName: "breakout::Scoreboard" };

type Velocity = {
  0: Vec3,
};
const Velocity: BevyType<Velocity> = { typeName: "breakout::Velocity" };

function run() {
  i++;
  if (i % 60 == 0) {
    let score = world.resource(Scoreboard)!;
    score.score += 1;
    info(score.score);
  }

  if (firstIteration) {
    firstIteration = false;

    for (const item of world.query(Transform, Velocity)) {
      let [transform, velocity] = item.components;
      info("Velocity:", velocity[0].toString());
      info("Transform:", transform.translation.toString());
    }
  }
}

export default {
  update: run,
};
