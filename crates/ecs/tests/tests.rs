use ecs::{
    entity::Entity,
    filter::Without,
    prelude::{ResMut, ScheduleBuilder},
    query::Query,
    world::World,
};
use ecs_derive::{Component, Resource, ScheduleLabel};
use tracing::Level;

#[derive(Debug, Copy, Clone, Component)]
struct Position {
    x: f32,
    y: f32,
}
#[derive(Debug, Copy, Clone, Component)]
struct Velocity {
    x: f32,
    y: f32,
}
#[derive(Debug, Copy, Clone, Component)]
struct Health(f32);
#[derive(Debug, Copy, Clone, Component)]
struct Faction(u8);
#[derive(Debug, Copy, Clone, Component)]
struct Mass(f32);
#[derive(Debug, Copy, Clone, Component)]
struct Static; // Marker component

#[derive(Debug, Resource)]
struct GlobalTimer(u32);

fn simple_system(query: Query<(Entity, &Health), Without<Mass>>) {
    for (entity, health) in &query {
        tracing::info!("Massless entity {} has {:?} health", entity.id(), health.0);

        assert!(!entity.has::<Mass>());
    }
}

fn second_system(query: Query<(&Health, &Mass)>) {
    for (health, mass) in &query {
        tracing::info!("health is {health:?}, mass is {mass:?}");
    }
}

fn resource_system(res: ResMut<GlobalTimer>) {
    let time = res.0;
    println!("Time is {time:?}");
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
        // .without_time()
        // .with_target(false)
        // .with_thread_names(true)
        // .with_file(true)
        // .with_line_number(true)
        .with_max_level(Level::TRACE)
        .compact()
        .init();

    let mut world = World::new();

    // Spawn 10,000 entities to ensure the loop actually takes time
    tracing::info!("Summoning entities");
    for i in 0..2u32 {
        world.spawn((
            // Position { x: i as f32, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            // Faction((i % 2) as u8),
            Mass(i as f32),
            Health(i as f32),
        ));
    }

    for i in 0..2u32 {
        world.spawn((
            // Faction((i % 2) as u8),
            Mass(i as f32 + 100.0),
            Health(i as f32 + 100.0),
        ));
    }

    world.add_resources(GlobalTimer(5));
    world.spawn(Health(69.0));

    tracing::info!("World has {} entities", world.entities().count());

    tracing::info!("Generating schedule...");
    let schedule = world
        .build_schedule()
        .add(Label1, (simple_system, second_system, resource_system))
        .schedule();

    world.run(&schedule);
}
