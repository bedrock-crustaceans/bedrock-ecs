use ecs::{component::Component, entity::Entity, filter::With, local::Local, query::Query, system::Systems, world::World};
use ecs::schedule::{ScheduleBuilder, SystemsLabel};

struct Alive {}

impl Component for Alive {}

struct Health(f32);

impl Component for Health {}

impl Drop for Health {
    fn drop(&mut self) {
        println!("Health dropped");
    }
}

fn system1(query: Query<&Health>, mut counter: Local<usize>) {
    for thing in &query {
        // println!("Health: {}, counter: {}", thing.0, *counter);
        // // thing.0 *= 2.0;
        // *counter += 1;

        // *LOCK.write() = Some(thing);
        // println!("id: {:?}", entity.id());
    }
}

fn system3(query: Query<(&mut Health, &Alive)>) {

}

fn system4(query: Query<(Entity, &Alive)>) {

}

fn system2(query: Query<&Alive>) {
    for alive in &query {

    }
}

struct Label1;

impl SystemsLabel for Label1 {
    const NAME: &'static str = "Label1";
}

struct Label2;

impl SystemsLabel for Label2 {
    const NAME: &'static str = "Label2";
}

#[test]
fn system_test() {
    let mut world = World::new();

    println!("spawn");
    world.spawn(Health(5.0));
    world.spawn(Health(1.0));
    // world.spawn(Health(0.5));

    let schedule = ScheduleBuilder::new()
        .add(Label1, (system1, system2))
        .add(Label2, (system3, system4))
        .schedule();

    println!("{schedule:?}");

    // println!("begin push");
    // world.systems.push(system1);
    //
    // println!("begin call");
    // world.systems.call(&world);
    // println!("end call");

    todo!();
}