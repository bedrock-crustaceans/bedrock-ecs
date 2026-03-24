use bedrock_ecs::command::Commands;
use bedrock_ecs::entity::EntityHandle;
use bedrock_ecs::message::{Message, MessageReceiver, MessageSender};
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
struct Example2;

#[derive(Component)]
struct Example3;

#[derive(Message, Debug, Clone)]
struct Killed {
    entity: EntityHandle,
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
    query: Query<
        &Name,
        Or<(
            Not<(With<Example1>, With<Example2>)>,
            Xor<(With<Example1>, With<Example2>)>,
        )>,
    >,
) {
    println!("{:?}", query.meta().cache());

    for name in &query {
        tracing::error!("found {}", name.0);
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

    world.spawn(Name("none"));
    world.spawn((Name("example3"), Example3));
    world.spawn((Name("example1+2"), Example1, Example2));
    world.spawn((Name("example2"), Example2));

    let schedule = world
        .build_schedule()
        // .add(Label1, (damage_system, detector, fall_system, reviver))
        .add(Label1, test_system)
        .schedule();

    world.run(&schedule);
}
