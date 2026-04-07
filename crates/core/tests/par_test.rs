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
    let sum = query.par_iter().map(|(pos, _)| pos.x).sum::<f32>();
    println!("sum is: {sum}");
}

#[test]
fn par_iter_test() {
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let fmt = tracing_subscriber::fmt::Layer::new()
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .compact()
        .with_filter(
            tracing_subscriber::filter::Targets::new()
                .with_target("bedrock_ecs", tracing::Level::TRACE),
        );

    tracing_subscriber::registry().with(fmt).init();

    let mut world = World::new();

    println!("Spawning entities");
    for i in 0..1_000 {
        world.spawn((
            Position {
                x: i as f32,
                y: 0.0,
                z: 0.0,
            },
            Velocity {
                x: i as f32 / 100_000.0,
                y: 0.1,
                z: 0.1,
            },
        ));
    }

    for i in 0..1_000 {
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
    println!("Running systems");

    let schedule = ScheduleBuilder::new(&mut world).add(Label, (par_sys));
    let mut scheduler = schedule.schedule();

    let ticks = 1;
    for _ in 0..ticks {
        scheduler.run(&mut world);
    }
}
