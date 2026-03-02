use crate::{component::Component, entity::Entity, local::Local, query::Query, system::Systems, world::World};

struct Health(f32);

impl Component for Health {}

impl Drop for Health {
    fn drop(&mut self) {
        println!("Health dropped");
    }
}

fn system1(query: Query<&Health>, mut counter: Local<usize>) {
    for thing in &query {
        println!("Health: {}, counter: {}", thing.0, *counter);
        // thing.0 *= 2.0;
        *counter += 1;

        // *LOCK.write() = Some(thing);
        // println!("id: {:?}", entity.id());
    }
}

#[test]
fn system_test() {
    let mut world = World::new();

    world.spawn(Health(5.0));
    world.spawn(Health(1.0));
    world.spawn(Health(0.5));

    world.systems.push(system1);

    world.systems.call(&world);

    todo!();
}