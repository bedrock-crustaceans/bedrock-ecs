use ecs::command::Commands;
use ecs::entity::Entity;
use ecs::{query::Query, world::World};
use ecs_derive::{Component, Resource, ScheduleLabel};
use tracing::Level;

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
    println!("start on thread {:?}", std::thread::current().id());

    for bytes in &query {
        std::thread::sleep(std::time::Duration::from_millis(1));
        // println!("bytes5: {} {}", b0, bytes.1);
    }

    println!("finish on thread {:?}", std::thread::current().id());
}

fn simple_system2(query: Query<(Entity, &mut Static)>, mut commands: Commands) {
    println!("start on thread {:?}", std::thread::current().id());

    for (entity, bytes) in &query {
        std::thread::sleep(std::time::Duration::from_millis(1));
        // let b0 = bytes.0;
        // println!("bytes5: {} {}", b0, bytes.1);

        // let entity2 = commands.spawn_empty();
        // tracing::trace!("is deferred: {}", entity2.deferred());

        // entity2.despawn();
        commands.entity(entity).despawn();
    }

    println!("finish on thread {:?}", std::thread::current().id());
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
        .with_thread_names(true)
        .with_max_level(Level::TRACE)
        .compact()
        .init();

    let mut world = World::new();

    for _ in 0..1 {
        world.spawn(Bytes5(5.0, 22));
        world.spawn((Bytes5(f32::MAX, u8::MAX), Static));
    }

    println!("added: {}", f32::MAX);

    let schedule = world
        .build_schedule()
        .add(Label1, (/*simple_system,*/simple_system2))
        .schedule();

    world.run(&schedule);
}
