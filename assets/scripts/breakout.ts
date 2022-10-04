let firstIteration = true;
let i = 0;

type Scoreboard = {
  score: number;
};
const Scoreboard: BevyType<Scoreboard> = { typeName: "breakout::Scoreboard" };

type Velocity = {
  0: Vec3;
};
const Velocity: BevyType<Velocity> = { typeName: "breakout::Velocity" };

type NotABallYet = unknown;
const NotABallYet: BevyType<NotABallYet> = {
  typeName: "breakout::NotABallYet",
};
type Ball = unknown;
const Ball: BevyType<Ball> = { typeName: "breakout::Ball" };

function run() {
  i++;
  if (i % 60 == 0) {
    let score = world.resource(Scoreboard)!;
    score.score += 1;
    info(score.score);
  }

  // Start the ball movement
  if (i == 60) {
    // Query the entity that has the NotABallYet component
    for (const item of world.query(NotABallYet)) {
      // Insert the ball component on that entity
      world.insert(item.entity, Value.default(Ball));

      // Create a velocity component
      let vel = Value.default(Velocity);
      // Set the velocity speed
      vel[0].x = -200.0;
      vel[0].y = 300.0;

      // Add the velocity to the ball
      world.insert(item.entity, vel);
    }
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
