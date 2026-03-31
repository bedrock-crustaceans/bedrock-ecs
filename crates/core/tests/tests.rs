use bedrock_ecs::command::Commands;
use bedrock_ecs::entity::Entity;
use bedrock_ecs::entity::EntityGeneration;
use bedrock_ecs::entity::EntityIndex;
use bedrock_ecs::query::Query;
use bedrock_ecs::query::Without;
use bedrock_ecs::scheduler::{ScheduleBuilder, ScheduleLabel, SystemBundle};
use bedrock_ecs::world::World;
use bedrock_ecs_derive::Component;

#[derive(Debug, Component)]
struct Position {
    x: f32,
    y: f32,
}
#[derive(Debug, Component)]
struct Velocity {
    x: f32,
    y: f32,
}
#[derive(Debug, Component)]
struct Health(f32);
#[derive(Debug, Component)]
struct Faction(u8);
#[derive(Debug, Component)]
struct Mass(f32);
#[derive(Debug, Component)]
struct Static; // Marker component
#[derive(Debug, Component)]
struct Stamina(f32);
#[derive(Debug, Component)]
struct Sprite {
    id: u32,
    visible: bool,
}
#[derive(Debug, Component)]
struct Target(Option<u32>);

// // Yes this was AI-generated. I could not be bothered to create a large amount of systems by hand.
// // --- 1. SENSOR SYSTEMS (Heavy Read-Only) ---
// // These should all run in parallel as they only borrow &Position
// fn proximity_sensor_system(query: Query<&Position>) { /* ... */ }
// fn visibility_check_system(query: Query<(&Position, &Sprite)>) { /* ... */ }
// fn audio_emitter_system(query: Query<(&Position, &Velocity)>) { /* ... */ }

// // --- 2. ATTRIBUTE SYSTEMS (Mixed Access) ---
// // These compete for Health and Stamina
// fn hunger_drain_system(query: Query<&mut Health>) { /* ... */ }
// fn fatigue_system(query: Query<(&Velocity, &mut Stamina)>) { /* ... */ }
// fn oxygen_system(query: Query<(&Position, &mut Health)>) { /* ... */ }

// // --- 3. AI & BEHAVIOR (Decision Making) ---
// // High contention on 'Target' and 'Faction'
// fn aggro_logic_system(query: Query<(&Faction, &mut Target)>) { /* ... */ }
// fn flee_logic_system(query: Query<(&Health, &mut Velocity)>) { /* ... */ }
// fn wander_idle_system(query: Query<&mut Velocity, Without<Target>>) { /* ... */ }

// // --- 4. COSMETIC & VFX (Late Frame Reads) ---
// // Read-heavy systems that usually run at the end of the frame
// fn particle_spawn_system(query: Query<(&Position, &Velocity)>) { /* ... */ }
// fn trail_renderer_system(query: Query<(&Position, &Sprite)>) { /* ... */ }
// fn debug_render_system(query: Query<(&Position, &Health)>) { /* ... */ }

// // // --- 5. THE DATA CRUSHER (Structural Stress) ---
// // // This system randomly attaches "Buff" components, forcing archetype migrations
// fn buff_applicator_system(query: Query<(Entity, &Health)>) { /* ... */ }

// // 1. Movement: Read Vel, Write Pos. (Parallel-friendly)
// fn movement_system(query: Query<(&Velocity, &mut Position)>) {
//     for (vel, mut pos) in &query {
//         pos.x += vel.x;
//         pos.y += vel.y;
//     }
// }

// // 2. Gravity: Read Mass, Write Vel. (Parallel-friendly)
// fn gravity_system(query: Query<(&Mass, &mut Velocity), Without<Static>>) {
//     for (_mass, vel) in &query {
//         vel.y -= 9.81;
//     }
// }

// fn combat_system(query: Query<(&Position, &Faction, &mut Health)>) {
//     // We use a nested loop to simulate the "Naive" approach.
//     // Note: In a real ECS, you would use a "View" or "Snapshot"
//     // to avoid double-borrowing the query while iterating.

//     let attack_range: f32 = 5.0;
//     let damage: f32 = 0.5;

//     // Collect positions and factions into a temporary buffer to avoid
//     // multiple mutable borrow conflicts during the nested loop.
//     let entities: Vec<(&Position, &Faction)> = query
//         .iter()
//         .map(|(pos, faction, _)| (pos, faction))
//         .collect();

//     // The Stress Maker: Nested Iteration
//     for (pos_a, faction_a, mut health_a) in &query {
//         for (pos_b, faction_b) in &entities {
//             // Only fight different factions
//             if faction_a.0 != faction_b.0 {
//                 let dx = pos_a.x - pos_b.x;
//                 let dy = pos_a.y - pos_b.y;
//                 let distance_sq = dx * dx + dy * dy;

//                 if distance_sq < attack_range * attack_range {
//                     // Apply damage
//                     health_a.0 -= damage;
//                 }
//             }
//         }
//     }
// }
// // 4. Regeneration: Write Health.
// fn regen_system(query: Query<&mut Health>) {
//     for health in &query {
//         health.0 = (health.0 + 0.1).min(100.0);
//     }
// }

// // 5. The "Wall": Mutates everything. This forces serial execution.
// fn collision_system(query: Query<(Entity, &mut Position, &mut Velocity, &Mass)>) {
//     // Expensive logic that moves entities back if they collide
// }

// // 6. Cleanup: Read Health, Command Entity destruction.
// fn death_system(query: Query<(Entity, &Health)>) {
//     for (entity, health) in &query {
//         if health.0 <= 0.0 { /* despawn logic */ }
//     }
// }

// fn animation_system(query: Query<(&Velocity, &mut Sprite)>) {
//     for (vel, mut sprite) in &query {
//         let speed_sq = vel.x * vel.x + vel.y * vel.y;

//         // Logic branching: checks if the entity is moving
//         if speed_sq > 0.01 {
//             sprite.visible = true;
//             // Cycle through a dummy animation sheet of 10 frames
//             sprite.id = (sprite.id + 1) % 10;
//         } else {
//             // Idle state: use frame 0 and potentially hide sprite
//             sprite.id = 0;
//             sprite.visible = (speed_sq % 2.0) > 1.0; // Flickering effect stress
//         }
//     }
// }

/// 1. Movement: Updates Position based on Velocity.
// fn physics_step_system(query: Query<(&mut Position, &Velocity), Without<Static>>) { /* ... */ }

/// 2. Gravity: Constant downward acceleration.
fn gravity_apply_system(query: Query<(&mut Velocity, &Mass), Without<Static>>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 3. Friction: Slows down Velocity over time.
fn air_resistance_system(query: Query<(&mut Velocity, &Position)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 4. Boundary: Bounce Velocity if Position hits edge.
fn map_bounds_system(query: Query<(&mut Position)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 5. Targeting: Scan for nearest Entity of different Faction.
fn ai_perception_system(query: Query<(Entity, &Position, &Faction, &mut Target)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 6. Homing: Steering Velocity toward Target.
fn target_tracking_system(query: Query<(&Target, &Position, &mut Velocity)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 7. Health: Natural regeneration if not moving.
fn health_regen_system(query: Query<(&mut Health, &Velocity, &Stamina)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 8. Stamina: Deplete Stamina based on Velocity magnitude.
fn stamina_drain_system(query: Query<(&mut Stamina, &Velocity)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 9. Stamina: Slow recovery over time.
fn stamina_recovery_system(query: Query<(&mut Stamina, &Health)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 10. Death: Mark Sprite invisible if Health <= 0.
fn death_cleanup_system(query: Query<(&Health, &mut Sprite)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 11. Visuals: Sync Sprite position to Position.
fn sprite_transform_system(query: Query<(&Position, &mut Sprite)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 12. Visuals: Flash Sprite if Health is low.
fn low_health_vfx_system(query: Query<(&Health, &mut Sprite)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 13. Combat: Poison nearby Factions.
fn poison_aura_system(query: Query<(&Position, &Faction, &mut Health)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 14. Combat: Lifesteal from Target.
fn vampiric_drain_system(query: Query<(&Target, &mut Health, &mut Stamina)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 15. Combat: Knockback based on Mass.
fn impact_physics_system(query: Query<(&mut Velocity, &Mass, &Health)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 16. Utility: Frozen status for Static entities.
fn static_marker_sync_system(query: Query<(&Velocity, &Static)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 17. UI: Update health bars (Reads Health/Position).
fn ui_health_bar_system(query: Query<(&Health, &Position)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 18. Sound: Play footstep sounds based on Velocity/Position.
fn footstep_audio_system(query: Query<(&Velocity, &Position, &mut Sprite)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 19. Buffs: Increase Mass if Stamina is high.
fn bulk_up_system(query: Query<(&Stamina, &mut Mass)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

/// 20. Debug: Teleport Target to random Position.
fn chaos_debug_system(query: Query<(&mut Target, &mut Position)>) {
    /* ... */
    let dur = rand::random_range(0..1000);
    std::thread::sleep(std::time::Duration::from_micros(dur));
}

fn test_system(
    query1: Query<Entity, Without<Mass>>,
    query2: Query<(Entity, &Mass)>,
    mut commands: Commands,
) {
    println!("query1 matches {:?} items", query1.size_hint());
    println!("query2 matches {} items", query2.iter().len());

    for entity in &query1 {
        println!(
            "adding mass {} to entity {}",
            entity.index(),
            entity.index()
        );

        let mut cmds = commands.spawn(Static);
        cmds.insert(Mass(42.0 + entity.index().to_bits() as f32));
    }

    for (entity, mass) in &query2 {
        println!("entity {} has mass {}", entity.index(), mass.0);
    }
}

struct Physics;

impl ScheduleLabel for Physics {
    const NAME: &'static str = "Physics";
}

struct Logic;

impl ScheduleLabel for Logic {
    const NAME: &'static str = "Logic";
}

struct Combat;

impl ScheduleLabel for Combat {
    const NAME: &'static str = "Combat";
}

struct Visuals;

impl ScheduleLabel for Visuals {
    const NAME: &'static str = "Visuals";
}

#[test]
fn stress_test() {
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .compact()
        .init();

    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()
        .unwrap();

    let mut world = World::new();

    // // Spawn 1000 entities to ensure the loop actually takes time
    // for i in 0..1000u32 {
    //     world.spawn((
    //         Position {
    //             x: i as f32,
    //             y: i as f32,
    //         },
    //         Velocity { x: 1.0, y: -1.0 },
    //         Health(100.0),
    //         Stamina(100.0),
    //         Faction((i % 2) as u8),
    //         Sprite {
    //             id: i,
    //             visible: true,
    //         },
    //         Target(Some(i + 1)),
    //     ));
    // }

    let mut schedule = ScheduleBuilder::new(&mut world)
        // --- PHASE 1: Physics & Movement (High Velocity/Position Contention) ---
        .add(
            Physics,
            (
                // physics_step_system,      // Write: Position, Read: Velocity
                gravity_apply_system,      // Write: Velocity, Read: Mass
                air_resistance_system,     // Write: Velocity, Read: Position
                map_bounds_system,         // Write: Velocity, Read: Position
                static_marker_sync_system, // Write: Velocity, Read: Static
                test_system,
            ),
        )
        // --- PHASE 2: Intelligence & Strategy (Target/Faction Logic) ---
        .add(
            Logic,
            (
                ai_perception_system,   // Write: Target, Read: Position, Faction
                target_tracking_system, // Write: Velocity, Read: Target, Position
                vampiric_drain_system,  // Write: Health, Stamina, Read: Target
                chaos_debug_system,     // Write: Target, Position
                bulk_up_system,         // Write: Mass, Read: Stamina
            ),
        )
        // --- PHASE 3: Vitality & Combat (Health/Stamina Updates) ---
        .add(
            Combat,
            (
                health_regen_system,     // Write: Health, Read: Velocity, Stamina
                stamina_drain_system,    // Write: Stamina, Read: Velocity
                stamina_recovery_system, // Write: Stamina, Read: Health
                poison_aura_system,      // Write: Health, Read: Position, Faction
                impact_physics_system,   // Write: Velocity, Read: Mass, Health
            ),
        )
        // --- PHASE 4: Visuals & Feedback (Sprite/UI/Audio) ---
        .add(
            Visuals,
            (
                death_cleanup_system,    // Write: Sprite, Read: Health
                sprite_transform_system, // Write: Sprite, Read: Position
                low_health_vfx_system,   // Write: Sprite, Read: Health
                ui_health_bar_system,    // Read: Health, Position
                footstep_audio_system,   // Write: Sprite, Read: Velocity, Position
            ),
        )
        .schedule();

    world.spawn(Static);

    // let mut schedule = ScheduleBuilder::new(&mut world)
    //     .add(Logic, test_system)
    //     .schedule();

    schedule.run(&world);
    world.apply_commands();
    // world.run(&schedule);

    let mut entity = world
        .get_entity_mut(Entity::from_index_and_generation(
            EntityIndex::from_bits(0),
            EntityGeneration::from_bits(0),
        ))
        .unwrap();

    let mass = entity.remove::<Mass>();
    println!("mass is: {mass:?}");

    let schedule_render = schedule.render_dependency_graph();

    std::fs::write("schedule.html", schedule_render)
        .expect("failed to write schedule render to file");

    let execution_render = schedule.render_execution_graph();
    std::fs::write("execution.html", execution_render)
        .expect("failed to write execution render to file");

    // open::that("schedule.html").unwrap();

    // // Execute loop
    // for _ in 0..100 {
    //     schedule.run(&mut world);
    // }
}
