use ecs::command::Commands;
use ecs::entity::Entity;
use ecs::message::{Message, MessageReceiver, MessageSender};
use ecs::query::Changed;
use ecs::{query::Query, world::World};
use ecs_derive::{Component, Message, Resource, ScheduleLabel};
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

#[derive(Message, Debug, Clone)]
struct Msg {
    hello: String,
}

fn simple_system(query: Query<&Bytes5>) {
    println!("start on thread {:?}", std::thread::current().id());

    for bytes in &query {
        std::thread::sleep(std::time::Duration::from_millis(1));
        // println!("bytes5: {} {}", b0, bytes.1);
    }

    println!("finish on thread {:?}", std::thread::current().id());
}

fn simple_system2(
    query: Query<&mut Bytes5>,
    mut commands: Commands,
    mut mailbox: MessageSender<Msg>,
) {
    commands.spawn(Static);
    for mut bytes in &query {
        bytes.1 -= 1;
    }

    mailbox.send(Msg {
        hello: "World".to_owned(),
    });
}

fn change_system(query: Query<&Bytes5, Changed<Bytes5>>, mailbox: MessageReceiver<Msg>) {
    for msg in mailbox {
        println!("message received: {msg:?}");
    }

    for bytes in &query {
        println!("bytes {bytes:?} changed");
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
        .add(Label1, (/*simple_system,*/ simple_system2, change_system))
        .schedule();

    world.run(&schedule);
    world.apply_commands();
}
