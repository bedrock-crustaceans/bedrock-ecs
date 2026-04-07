use bedrock_ecs::{prelude::ScheduleBuilder, query::Query, world::World};
use bedrock_ecs_derive::{Component, ScheduleLabel};
use rayon::iter::ParallelIterator;

#[derive(Component, Debug, Clone)]
struct Position {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Component, Debug, Clone)]
struct Velocity {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Component, Debug, Clone)]
struct Rotation {
    angle: f32,
}

#[derive(ScheduleLabel)]
struct Label;

fn par_sys(query: Query<(&mut Position, &Velocity)>) {
    query.par_iter().for_each(|(pos, vel)| {
        for _ in 0..50 {
            pos.x += vel.x.sin() * pos.y.cos();
            pos.y += vel.y.cos() * pos.z.sin();
            pos.z += (pos.x + pos.y).sqrt();
        }
    });
}

#[test]
fn par_iter_test() {
    let mut world = World::new();

    for i in 0..500_000 {
        world.spawn((
            Position {
                x: i as f32 / 10_000.0,
                y: i as f32 / 10_000.0,
                z: 0.0,
            },
            Velocity {
                x: i as f32 / 100_000.0,
                y: 0.1,
                z: 0.1,
            },
        ));
    }

    for _ in 0..1_000 {
        world.spawn((
            Position {
                x: i as f32,
                y: 1.0,
                z: 1.0,
            },
            Velocity {
                x: 0.1,
                y: 0.1,
                z: 0.1,
            },
            Rotation { angle: 0.1 },
        ));
    }

    let schedule = ScheduleBuilder::new(&mut world).add(Label, (par_sys));
    let mut scheduler = schedule.schedule();

    let ticks = 10;
    for _ in 0..ticks {
        scheduler.run(&mut world);
    }
}
