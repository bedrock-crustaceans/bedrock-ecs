use std::sync::RwLock;
use ecs_derive::Component;
use std::time::{Duration, Instant};

use crate::entity::Entity;
use crate::{
    Component, Event, EventReader, EventWriter, Query, Res, ResMut, Resource, Local, Without, World,
};

static GLOBAL: RwLock<Option<&'static Health>> = RwLock::new(None);

fn detection(query: Query<(Entity, &Health), Without<Immortal>>, mut writer: EventWriter<Killed>) {
    for (entity, health) in &query {
        if health.0 <= 0.0 {
            writer.write(Killed { entity });
        }
    }
}

fn execution(mut reader: EventReader<Killed>, mut counter: ResMut<KillCounter>) {
    for event in reader.read() {
        counter.0 += 1;
        println!(
            "Entity {:?} has been killed. {} entities killed so far",
            event.entity.id(),
            counter.0
        );
        event.entity.despawn();
    }
}

fn reader(mut reader: EventReader<Interval>, counter: Res<KillCounter>) {
    println!("Events: {}", reader.len());

    for _ in reader.read() {
        println!("{} entities have been killed so far", counter.0);
    }
}

fn interval_system(query: Query<&mut LastUpdate>, mut writer: EventWriter<Interval>) {
    let update = query.into_iter().next().unwrap();

    if Instant::now().duration_since(update.instant) > Duration::from_millis(1000) {
        update.instant = Instant::now();
        writer.write(Interval);
    }
}

#[derive(Default)]
struct SystemState {
    counter: usize,
}

fn state_system(mut state: Local<SystemState>) {
    state.counter += 1;
    println!("Counter is: {}", state.counter);
}

#[derive(Default)]
struct TickCounter {
    ticks: usize,
}

async fn async_system(
    mut reader: EventReader<Killed>,
    counter: Res<KillCounter>,
    mut state: Local<TickCounter>,
) {
    tokio::time::sleep(Duration::from_secs(1)).await;
    state.ticks += 1;

    println!(
        "After {} second(s), killed {} entities",
        state.ticks, counter.0
    );
    for Killed { entity } in reader.read() {
        println!("Killed {:?}", entity.id());
    }
}

#[derive(Debug, Component)]
struct LastUpdate {
    instant: Instant,
}

#[derive(Debug, Copy, Clone)]
struct Interval;

impl Event for Interval {}

#[derive(Debug)]
struct KillCounter(u32);

impl Resource for KillCounter {}

#[derive(Debug, Component)]
struct Immortal;

#[derive(Debug, Component)]
struct Health(f32);

#[derive(Clone)]
struct Killed {
    entity: Entity,
}

impl Event for Killed {}

#[tokio::test]
async fn ecs_test() {
    let world = World::new();

    // let pinned = boxer1(interval_system);
    // world.add_system(pinned);

    world.spawn(Health(0.0));
    world.spawn(Health(1.0));
    world.spawn(Health(0.0));
    world.spawn((Health(0.0), Immortal));
    world.spawn(LastUpdate {
        instant: Instant::now(),
    });
    world.add_resource(KillCounter(0));

    let mut schedule = world.schedule_single_threaded();

    schedule.add_system(interval_system);
    schedule.add_system(reader);
    schedule.add_system(detection);
    schedule.add_system(execution);
    schedule.add_system(state_system);
    schedule.add_async_system(async_system);

    let mut interval = tokio::time::interval(Duration::from_millis(50));
    for _ in 0..2 {
        schedule.run().await;
        interval.tick().await;
    }
}
