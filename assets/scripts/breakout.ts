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
      world.insert(item.entity, Value.create(Ball));

      // Create a velocity component
      //
      // We can optionally include a patch to the default value of velocity as the second argument
      // to create().
      let vel = Value.create(Velocity, [
        {
          x: -200,
        },
      ]);

      // For demonstration purposes, we can also patch values after they have been created. This
      // works on any ECS component, not just ones created with Value.create().
      Value.patch(vel, [
        {
          y: 200,
        },
      ]);

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
