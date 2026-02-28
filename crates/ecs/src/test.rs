use parking_lot::RwLock;

use crate::{entity::Entity, local::Local, query::Query, system::Systems, world::World};

fn system1(query: Query<Entity>) {
    for entity in &query {
        println!("id: {:?}", entity.id());
    }
}

fn system2(mut counter: Local<usize>) {
    // *counter += 1;
}

#[test]
fn system_test() {
    let mut world = World::new();

    world.systems.push::<Query<'_, Entity>, _>(system1);
    world.systems.push::<Local<usize>, _>(system2);

    world.systems.call(&world);

    todo!();
}