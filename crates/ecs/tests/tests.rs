use ecs::component::Component;
use ecs::entity::Entity;
use ecs::filter::Without;
use ecs::query::Query;
use ecs::schedule::{ScheduleBuilder, ScheduleLabel, SystemBundle};
use ecs::world::World;

struct Position { x: f32, y: f32 }
struct Velocity { x: f32, y: f32 }
struct Health(f32);
struct Faction(u8);
struct Mass(f32);
struct Static; // Marker component
struct Stamina(f32);
struct Sprite {
    id: u32, visible: bool
}
struct Target(Option<u32>);

impl Component for Position {}
impl Component for Velocity {}
impl Component for Health {}
impl Component for Faction {}
impl Component for Mass {}
impl Component for Static {}
impl Component for Stamina {}
impl Component for Sprite {}

impl Component for Target {}

// Yes this was AI-generated. I could not be bothered to create a large amount of systems by hand.
// --- 1. SENSOR SYSTEMS (Heavy Read-Only) ---
// These should all run in parallel as they only borrow &Position
fn proximity_sensor_system(query: Query<&Position>) { /* ... */ }
fn visibility_check_system(query: Query<(&Position, &Sprite)>) { /* ... */ }
fn audio_emitter_system(query: Query<(&Position, &Velocity)>) { /* ... */ }

// --- 2. ATTRIBUTE SYSTEMS (Mixed Access) ---
// These compete for Health and Stamina
fn hunger_drain_system(query: Query<&mut Health>) { /* ... */ }
fn fatigue_system(query: Query<(&Velocity, &mut Stamina)>) { /* ... */ }
fn oxygen_system(query: Query<(&Position, &mut Health)>) { /* ... */ }

// --- 3. AI & BEHAVIOR (Decision Making) ---
// High contention on 'Target' and 'Faction'
fn aggro_logic_system(query: Query<(&Faction, &mut Target)>) { /* ... */ }
fn flee_logic_system(query: Query<(&Health, &mut Velocity)>) { /* ... */ }
fn wander_idle_system(query: Query<&mut Velocity, Without<Target>>) { /* ... */ }

// --- 4. COSMETIC & VFX (Late Frame Reads) ---
// Read-heavy systems that usually run at the end of the frame
fn particle_spawn_system(query: Query<(&Position, &Velocity)>) { /* ... */ }
fn trail_renderer_system(query: Query<(&Position, &Sprite)>) { /* ... */ }
fn debug_render_system(query: Query<(&Position, &Health)>) { /* ... */ }

// --- 5. THE DATA CRUSHER (Structural Stress) ---
// This system randomly attaches "Buff" components, forcing archetype migrations
fn buff_applicator_system(query: Query<(Entity, &Health)>) { /* ... */ }

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

fn combat_system(query: Query<(&Position, &Faction, &mut Health)>) {
    // We use a nested loop to simulate the "Naive" approach.
    // Note: In a real ECS, you would use a "View" or "Snapshot"
    // to avoid double-borrowing the query while iterating.

    let attack_range: f32 = 5.0;
    let damage: f32 = 0.5;

    // Collect positions and factions into a temporary buffer to avoid
    // multiple mutable borrow conflicts during the nested loop.
    let entities: Vec<(&Position, &Faction)> = query
        .iter()
        .map(|(pos, faction, _)| (pos, faction))
        .collect();

    // The Stress Maker: Nested Iteration
    for (pos_a, faction_a, mut health_a) in &query {
        for (pos_b, faction_b) in &entities {
            // Only fight different factions
            if faction_a.0 != faction_b.0 {
                let dx = pos_a.x - pos_b.x;
                let dy = pos_a.y - pos_b.y;
                let distance_sq = dx * dx + dy * dy;

                if distance_sq < attack_range * attack_range {
                    // Apply damage
                    health_a.0 -= damage;
                }
            }
        }
    }
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

fn animation_system(query: Query<(&Velocity, &mut Sprite)>) {
    for (vel, mut sprite) in &query {
        let speed_sq = vel.x * vel.x + vel.y * vel.y;

        // Logic branching: checks if the entity is moving
        if speed_sq > 0.01 {
            sprite.visible = true;
            // Cycle through a dummy animation sheet of 10 frames
            sprite.id = (sprite.id + 1) % 10;
        } else {
            // Idle state: use frame 0 and potentially hide sprite
            sprite.id = 0;
            sprite.visible = (speed_sq % 2.0) > 1.0; // Flickering effect stress
        }
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

struct Label4;

impl ScheduleLabel for Label4 {
    const NAME: &'static str = "Label4";
}

#[test]
fn stress_test() {
    let mut world = World::new();

    // Spawn 1000 entities to ensure the loop actually takes time
    for i in 0..1000u32 {
        world.spawn((
            Position { x: i as f32, y: i as f32 },
            Velocity { x: 1.0, y: -1.0 },
            Health(100.0),
            Stamina(100.0),
            Faction((i % 2) as u8),
            Sprite { id: i, visible: true },
            Target(Some(i + 1))
        ));
    }

    let schedule = ScheduleBuilder::new()
        // PHASE A: Parallel Inputs & Sensors
        // All of these should run simultaneously if the scheduler is efficient.
        .add(Label1, (
            gravity_system,
            movement_system,
            proximity_sensor_system,
            visibility_check_system,
            audio_emitter_system
        ))

        // PHASE B: Logic Contention (The Bottleneck)
        // Combat, Aggro, and Flee all fight over Velocity/Health/Target.
        .add(Label2, (
            combat_system,
            aggro_logic_system,
            flee_logic_system,
            wander_idle_system,
            fatigue_system
        ))

        // PHASE C: Resource Management & Mutation
        // Forces structural changes and final attribute calculations.
        .add(Label3, (
            regen_system,
            hunger_drain_system,
            oxygen_system,
            buff_applicator_system,
            death_system
        ))

        // PHASE D: Post-Processing
        .add(Label4, (
            animation_system,
            particle_spawn_system,
            trail_renderer_system,
            debug_render_system
        ))
        .schedule();

    println!("{schedule:?}");

    world.run(&schedule);

    // // Execute loop
    // for _ in 0..100 {
    //     schedule.run(&mut world);
    // }
}