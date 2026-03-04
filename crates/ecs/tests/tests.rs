use ecs::component::Component;
use ecs::entity::Entity;
use ecs::filter::Without;
use ecs::query::Query;
use ecs::schedule::{ScheduleBuilder, ScheduleLabel};
use ecs::world::World;

struct Position { x: f32, y: f32 }
struct Velocity { x: f32, y: f32 }
struct Health(f32);
struct Faction(u8);
struct Mass(f32);
struct Static; // Marker component

impl Component for Position {}
impl Component for Velocity {}
impl Component for Health {}
impl Component for Faction {}
impl Component for Mass {}
impl Component for Static {}

// 1. Movement: Read Vel, Write Pos. (Parallel-friendly)
fn movement_system(query: Query<(&Velocity, &mut Position)>) {
    for (vel, mut pos) in &query {
        pos.x += vel.x;
        pos.y += vel.y;
    }
}

// 2. Gravity: Read Mass, Write Vel. (Parallel-friendly)
fn gravity_system(query: Query<(&Mass, &mut Velocity), Without<Static>>) {
    for (_mass, vel) in &query {
        vel.y -= 9.81;
    }
}

// 3. Combat: Read Faction/Pos, Write Health. (Heavy Read)
fn combat_system(query: Query<(&Position, &Faction, &mut Health)>) {
    // Logic for checking nearby enemies and applying damage
}

// 4. Regeneration: Write Health.
fn regen_system(query: Query<&mut Health>) {
    for health in &query {
        health.0 = (health.0 + 0.1).min(100.0);
    }
}

// 5. The "Wall": Mutates everything. This forces serial execution.
fn collision_system(query: Query<(Entity, &mut Position, &mut Velocity, &Mass)>) {
    // Expensive logic that moves entities back if they collide
}

// 6. Cleanup: Read Health, Command Entity destruction.
fn death_system(query: Query<(Entity, &Health)>) {
    for (entity, health) in &query {
        if health.0 <= 0.0 { /* despawn logic */ }
    }
}

struct Label1;

impl ScheduleLabel for Label1 {
    const NAME: &'static str = "Label1";
}

struct Label2;

impl ScheduleLabel for Label2 {
    const NAME: &'static str = "Label2";
}

struct Label3;

impl ScheduleLabel for Label3 {
    const NAME: &'static str = "Label3";
}

#[test]
fn stress_test() {
    let mut world = World::new();

    // Spawn 10,000 entities to ensure the loop actually takes time
    for i in 0..10000u32 {
        world.spawn((
            Position { x: i as f32, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            Health(100.0),
            Faction((i % 2) as u8),
            Mass(1.0),
        ));
    }

    let schedule = ScheduleBuilder::new()
        // Stage 1: High Parallelism (Physics + Combat)
        .add(Label1, (movement_system, gravity_system, combat_system))
        // Stage 2: Mixed (Regen + Death check)
        .add(Label2, (regen_system, death_system))
        // Stage 3: The Bottleneck (Collision logic)
        .add(Label3, collision_system)
        .schedule();

    println!("{:#?}", schedule);

    // // Execute loop
    // for _ in 0..100 {
    //     schedule.run(&mut world);
    // }
}