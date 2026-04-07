use bedrock_ecs::{
    entity::{Entity, EntityGeneration, EntityIndex},
    prelude::ScheduleBuilder,
    query::{Added, Query},
    world::World,
};
use bedrock_ecs_derive::{Component, ScheduleLabel};

#[derive(Component, Debug)]
struct Counter(i32);

#[derive(ScheduleLabel)]
struct Label;

fn counter_system1(query: Query<&mut Counter, Added<Counter>>) {
    for mut counter in &query {
        println!("+1");
        counter.0 += 1;
    }
}

fn counter_reader(query: Query<(Entity, &Counter)>) {
    for (entity, counter) in &query {
        println!("{:?} counter is now {counter:?}", entity.index().to_bits());
    }
}

#[test]
fn small_test() {
    let mut world = World::new();

    for i in 0..5 {
        world.spawn(Counter(i));
    }

    let schedule = ScheduleBuilder::new(&mut world).add(Label, (counter_system1, counter_reader));
    let mut scheduler = schedule.schedule();

    let ticks = 10;
    for _ in 0..ticks {
        scheduler.run(&mut world);
    }

    for i in 0..5 {
        let entity = Entity::from_index_and_generation(
            EntityIndex::from_bits(i),
            EntityGeneration::from_bits(0),
        );

        let counter = world.get_entity_mut(entity).unwrap().remove::<Counter>();
        println!("end counter {i} is: {counter:?}");
    }
}
