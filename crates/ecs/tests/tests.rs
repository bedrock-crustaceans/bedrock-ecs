use ecs::component::Component;
use ecs::entity::Entity;
use ecs::filter::Without;
use ecs::query::Query;
use ecs::schedule::{ScheduleBuilder, ScheduleLabel};
use ecs::world::World;

#[derive(Debug, Copy, Clone)]
struct Position { x: f32, y: f32 }
#[derive(Debug, Copy, Clone)]
struct Velocity { x: f32, y: f32 }
#[derive(Debug, Copy, Clone)]
struct Health(f32);
#[derive(Debug, Copy, Clone)]
struct Faction(u8);
#[derive(Debug, Copy, Clone)]
struct Mass(f32);
#[derive(Debug, Copy, Clone)]
struct Static; // Marker component

impl Component for Position {}
impl Component for Velocity {}
impl Component for Health {}
impl Component for Faction {}
impl Component for Mass {}
impl Component for Static {}

// fn movement_system(query: Query<(&Velocity, &mut Position)>) {
//     println!("movement_system called");

//     for (vel, mut pos) in &query {
//         // Force floating point unit (FPU) stress
//         let speed = (vel.x * vel.x + vel.y * vel.y).sqrt().max(0.001);
//         let modifier = (pos.x * 0.01).sin() * (pos.y * 0.01).cos();
        
//         pos.x += (vel.x / speed) * (1.0 + modifier);
//         pos.y += (vel.y / speed) * (1.0 + modifier);
//     }
// }

// fn gravity_system(query: Query<(&Mass, &mut Velocity), Without<Static>>) {
//     println!("gravity_system called");

//     for (mass, mut vel) in &query {
//         // Simulated orbital gravity pull towards center (0,0)
//         let dist_sq = 100.0f32.max(1.0); 
//         vel.y -= (9.81 * mass.0) / dist_sq;
//         vel.x += 0.01; // Constant cross-wind stress

//         println!("Velocity is {vel:?}");
//     }
// }

// fn combat_system(query: Query<(&Position, &Faction, &mut Health)>) {
//     println!("combat_system called");

//     // Collecting to a Vec is a common ECS stressor: it tests allocation 
//     // and linear iteration outside of the ECS storage.
//     let entities: Vec<_> = query.iter().map(|(p, f, _)| (*p, *f)).collect();

//     for (pos_a, faction_a, health_a) in &query {
//         for (pos_b, faction_b) in &entities {
//             if faction_a.0 != faction_b.0 {
//                 let dx = pos_a.x - pos_b.x;
//                 let dy = pos_a.y - pos_b.y;
//                 if dx*dx + dy*dy < 25.0 { // Radius of 5.0
//                     health_a.0 -= 0.1;
//                 }
//             }
//         }
//     }
// }

// fn collision_system(query: Query<(Entity, &mut Position, &mut Velocity, &Mass)>) {
//     println!("collision_system called");

//     let entities: Vec<_> = query.iter().map(|(e, p, _, m)| (e, *p, m.0)).collect();

//     for (entity_a, mut pos_a, mut vel_a, mass_a) in &query {
//         for (entity_b, pos_b, mass_b) in &entities {
//             if entity_a.id() == entity_b.id() { continue; }

//             let dx = pos_a.x - pos_b.x;
//             let dy = pos_a.y - pos_b.y;
//             let dist_sq = dx*dx + dy*dy;

//             if dist_sq < 1.0 {
//                 // Elastic collision math (Expensive)
//                 let dist = dist_sq.sqrt().max(0.001);
//                 let nx = dx / dist;
//                 let ny = dy / dist;
                
//                 // Bounce back
//                 vel_a.x += nx * mass_b;
//                 vel_a.y += ny * mass_b;
//                 pos_a.x += nx * 0.1;
//                 pos_a.y += ny * 0.1;
//             }
//         }
//     }
// }

// fn regen_system(query: Query<&mut Health>) {
//     println!("regen_system called");

//     for mut health in &query {
//         // High-frequency small updates
//         health.0 = (health.0 + 0.05).min(100.0);
//     }
// }

// fn death_system(query: Query<(Entity, &Health)>) {
//     println!("death_system called");

//     for (entity, health) in &query {
//         if health.0 <= 0.0 {
//             // In a real ECS stress test, this would trigger 
//             // a command to despawn the entity, testing the 
//             // structural change overhead.
//             let _ = entity; 
//         }
//     }
// }

fn simple_system(query: Query<(Entity, &Health)>) {
    for component in &query {
        println!("{:?}", component.1);
    }
}

fn second_system(query: Query<(&Health, &Mass)>) {
    for (health, mass) in &query {
        println!("health is {health:?}, mass is {mass:?}");
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
    println!("Summoning entities");
    for i in 0..2u32 {
        world.spawn((
            // Position { x: i as f32, y: 0.0 },
            Velocity { x: 1.0, y: 1.0 },
            // Faction((i % 2) as u8),
            Mass(1.0),
            Health(i as f32),
        ));
    }

    world.spawn(Health(12.0));

    println!("World has {} entities", world.entities().count());

    println!("Generating schedule...");
    let schedule = ScheduleBuilder::new(&mut world)
        // // Stage 1: High Parallelism (Physics + Combat)
        // .add(Label1, (movement_system, gravity_system, combat_system))
        // // Stage 2: Mixed (Regen + Death check)
        // .add(Label2, (regen_system, death_system))
        // // Stage 3: The Bottleneck (Collision logic)
        // .add(Label3, collision_system)
        .add(Label1, (simple_system, second_system))
        .schedule();

    world.run(&schedule);

    // // Execute loop
    // for _ in 0..100 {
    //     schedule.run(&mut world);
    // }
}