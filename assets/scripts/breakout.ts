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

type Ball = unknown;
const Ball: BevyType<Ball> = { typeName: "breakout::Ball" };

type KeyCode = unknown; // enum handling is not implemented
const KeyCode: BevyType<KeyCode> = { typeName: "bevy_input::keyboard::KeyCode" };

type Input<T> = {
  pressed: (key: T) => boolean,
  just_pressed: (key: T) => boolean,
  press: (key: T) => void,
  get_pressed: () => T[],
};
const Input: <T>(T: BevyType<T>) => BevyType<Input<T>> = (T) => ({
  typeName: `bevy_input::input::Input<${T.typeName}>`,
});

function run() {
  i++;
  if (i % 60 == 0) {
    let score = world.resource(Scoreboard)!;
    score.score += 1;
    // info(score.score);
  }

  // let input = world.resource(Input(KeyCode))!;
  // let pressed = input.get_pressed();
  // info(pressed.toString());


  if (firstIteration) {
    firstIteration = false;

    // let value = Value.create(Transform, {
    //   translation: { x: 5.0, y: 2.0 }
    // });
    // info(value.toString());

    /*for (const item of world.query(Transform, Velocity)) {
      let [transform, velocity] = item.components;

      info("Velocity:", velocity[0].toString());
      info("Transform:", transform.translation.toString());
    }*/
  }
}

export default {
  update: run,
};
