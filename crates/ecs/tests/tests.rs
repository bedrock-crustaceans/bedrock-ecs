use ecs::{
    entity::{EntityRef, EntityHandle},
    filter::Without,
    prelude::{ResMut, ScheduleBuilder},
    query::Query,
    world::World,
};
use ecs_derive::{Component, Resource, ScheduleLabel};
use tracing::Level;
use ecs::command::Commands;
use ecs::entity::Entity;
use ecs::filter::With;

#[derive(Debug, Copy, Clone, Component)]
struct Velocity {
    x: f32,
    y: f32,
}
#[derive(Debug, Copy, Clone, Component)]
struct Health(f32);

#[derive(Debug, Copy, Clone, Component)]
struct Bytes5(f32, u8);

#[derive(Debug, Copy, Clone, Component)]
struct Static; // Marker component

#[derive(Debug, Resource)]
struct GlobalTimer(u32);

fn simple_system(query: Query<&Bytes5>) {
    for bytes in &query {
        let b0 = bytes.0;
        println!("bytes5: {} {}", b0, bytes.1);
    }
}

#[derive(ScheduleLabel)]
struct Label1;

#[derive(ScheduleLabel)]
struct Label2;

#[derive(ScheduleLabel)]
struct Label3;

#[test]
fn stress_test() {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .compact()
        .init();

    let mut world = World::new();

    world.spawn(Bytes5(5.0, 22));
    world.spawn(Bytes5(f32::MAX, u8::MAX));

    println!("added: {}", f32::MAX);

    let schedule = world
        .build_schedule()
        .add(Label1, simple_system)
        .schedule();

    world.run(&schedule);
}
