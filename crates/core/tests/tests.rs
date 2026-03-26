use bedrock_ecs::command::Commands;
use bedrock_ecs::entity::{Entity, EntityGeneration, EntityIndex};
use bedrock_ecs::message::{Inbox, Message, Outbox};
use bedrock_ecs::query::{Added, Changed, Has, Not, Or, With, Without, Xor};
use bedrock_ecs::time::SystemTick;
use bedrock_ecs::{query::Query, world::World};
use bedrock_ecs_derive::{Component, Message, Resource, ScheduleLabel};
use rustc_hash::FxHashMap;
use tracing::Level;

#[derive(Debug, Copy, Clone, Component)]
struct Name(&'static str);

#[derive(Debug, Copy, Clone, Component)]
struct Health(f32);

#[derive(Component)]
struct Example1;

#[derive(Component)]
struct Example2 {
    hello: String,
}

#[derive(Component)]
struct Example3;

#[derive(Component)]
struct AddedComp;

#[derive(Message, Debug, Clone)]
struct Killed {
    entity: Entity,
}

// fn detector(query: Query<(Entity, &Health), Changed<Health>>, mut sender: MessageSender<Killed>) {
//     for (entity, health) in &query {
//         tracing::trace!("detector triggered");
//         if health.0 <= 0.0 {
//             tracing::trace!("Entity death sent");
//             sender.send(Killed {
//                 entity: entity.handle,
//             });
//         }
//     }
// }

// fn damage_system(query: Query<(&Position, &mut Health)>) {
//     for (position, mut health) in &query {
//         if position.y <= 10.0 {
//             tracing::trace!("Entity damaged");
//             health.0 -= 1.0;
//         } else {
//             tracing::trace!("Entity unscathed");
//         }
//     }
// }

// fn fall_system(query: Query<(&mut Position, Has<Health>)>) {
//     for (mut position, has_health) in &query {
//         position.y -= 1.0;

//         if position.y < 10.0 {
//             position.y = 12.0;
//         }
//     }
// }

// fn reviver(recv: MessageReceiver<Killed>, tick: SystemTick) {
//     for msg in recv {
//         tracing::trace!("entity death received in tick {:?}", tick.this_run());
//     }
// }

fn test_system(
    query: Query<Entity, Without<Example2>>,
    query2: Query<(Entity, &Example2)>,
    mut commands: Commands,
) {
    for ent in &query {
        commands.entity(ent).insert(Example2 {
            hello: "world".to_owned(),
        });
        println!("adding example2 component");
    }

    for (ent, ex) in &query2 {
        commands.entity(ent).despawn();
        println!("queued despawn");
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

    world.spawn(Example1);

    let schedule = world
        .build_schedule()
        // .add(Label1, (damage_system, detector, fall_system, reviver))
        .add(Label1, test_system)
        .schedule();

    println!("first run");
    world.run(&schedule);
    world.apply_commands();
    println!("second run");
    world.run(&schedule);
    world.apply_commands();
    println!("third run");
    world.run(&schedule);
}
