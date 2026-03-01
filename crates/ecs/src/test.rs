use parking_lot::RwLock;

use crate::{component::Component, entity::Entity, local::Local, query::Query, system::Systems, world::World};

struct Health(f32);

impl Component for Health {}

fn system1(query: Query<&mut Health>, counter: Local<usize>) {
    for thing in &query {
        // *LOCK.write() = Some(thing);
        // println!("id: {:?}", entity.id());
    }
}

fn system2(mut counter: Local<usize>) {
    println!("Counter is: {}", *counter);
    *counter += 1;
}

#[test]
fn system_test() {
    let mut world = World::new();

    let entity = world.spawn(Health(5.0));

    world.systems.push(system1);
    world.systems.push(system2);

    world.systems.call(&world);
    world.systems.call(&world);
    world.systems.call(&world);
    world.systems.call(&world);

    todo!();
}