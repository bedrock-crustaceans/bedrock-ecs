use bedrock_ecs::{
    command::Commands,
    entity::Entity,
    plugins::PluginRegistry,
    prelude::{Res, ResMut, ScheduleBuilder},
    query::{Query, Without},
    world::World,
};
use bedrock_ecs_derive::{Component, Resource, ScheduleLabel};
use rand::prelude::*;
use wasmtime::{Caller, Engine, Linker, Module, Store};

#[derive(Debug, Copy, Clone, Component)]
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

#[derive(ScheduleLabel)]
struct Physics;

#[derive(ScheduleLabel)]
struct Logic;

#[derive(ScheduleLabel)]
struct Combat;

#[derive(ScheduleLabel)]
struct Visuals;

use std::collections::HashMap;

#[derive(Debug, Resource, Default)]
struct SpatialGrid {
    // Maps a grid coordinate (x, y) to a list of entity IDs and their factions
    cells: HashMap<(i32, i32), Vec<(Entity, Position, u8)>>,
    cell_size: f32,
}

impl SpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cells: HashMap::new(),
            cell_size,
        }
    }

    fn pos_to_cell(&self, pos: &Position) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }
}

// --- 1. THE STRESS WORKER ---
// A helper to simulate CPU work that can't be optimized away
fn spin_work(iters: usize) -> f32 {
    (0..iters).map(|i| (i as f32).sqrt()).sum()
}

/// Stress: High-Volume Read/Write
/// Targets the core "Movement" bottleneck of a Minecraft server.
fn physics_step_system(query: Query<(&mut Position, &Velocity), Without<Static>>) {
    query.iter().for_each(|(mut pos, vel)| {
        pos.x += vel.x;
        pos.y += vel.y;
        // Simulate complex collision math
        spin_work(50);
    });
}

/// Stress: Nested Querying (O(n^2) simulation)
/// Scans for nearby entities. This tests how your ECS handles multiple
/// simultaneous borrows of the same component type.
fn ai_perception_system(query: Query<(Entity, &Position, &Faction)>, targets: Query<&mut Target>) {
    // Collect positions into a flat buffer for "spatial" lookup simulation
    let entities: Vec<(Entity, Position, u8)> =
        query.iter().map(|(e, p, f)| (e, *p, f.0)).collect();

    targets.iter().for_each(|mut target| {
        // Simple heuristic: look for the "closest" entity in the buffer
        // In a real server, this would be a Kd-Tree or HashGrid lookup
        if let Some(first_e) = entities.first() {
            target.0 = Some(first_e.0.index().to_bits());
        }
        spin_work(100);
    });
}

/// Stress: Structural Changes (The "Archetype Killer")
/// Randomly adds/removes components, forcing the ECS to move entities
/// between memory chunks. This is the ultimate test for your Scheduler's Commands buffer.
fn chaos_spawner_system(mut commands: Commands, query: Query<(Entity, &Health), Without<Static>>) {
    let mut rng = rand::rng();

    for (entity, health) in &query {
        if health.0 <= 0.0 {
            commands.entity(entity).despawn();
            // tracing::warn!("Despawned {entity:?}");

            // Respawn a new one to keep entity count stable
            commands.spawn((
                Position {
                    x: rng.random(),
                    y: rng.random(),
                },
                Velocity { x: 0.1, y: 0.1 },
                Health(100.0),
                Faction(rng.random_range(0..3)),
                Stamina(100.0),
                Sprite {
                    id: 0,
                    visible: true,
                },
                Target(None),
            ));
        } else if rng.random_ratio(1, 10) {
            // Randomly "Stun" entities by adding the Static marker
            // tracing::warn!("Stunned {entity:?}");
            commands.entity(entity).insert(Static);
        }
    }
}

/// Stress: Heavy Writing (Regen & Poison)
/// Tests write-lock contention across different systems.
fn vitality_system(query: Query<(&mut Health, &mut Stamina, &Position)>) {
    query.iter().for_each(|(mut health, mut stamina, _pos)| {
        health.0 = (health.0 + 0.01).min(100.0);
        stamina.0 = (stamina.0 - 0.02).max(0.0);
        spin_work(20);
    });
}

fn gravity_apply_system(query: Query<(&mut Velocity, &Mass), Without<Static>>) {
    println!("gravity");

    const GRAVITY_CONSTANT: f32 = -0.08; // Bedrock-ish gravity scale

    query.iter().for_each(|(mut vel, mass)| {
        // Heavy objects might fall faster or resist air differently
        // We simulate a basic terminal velocity calculation
        let force = GRAVITY_CONSTANT * mass.0;
        vel.y += force;

        // Cap terminal velocity to prevent "ghosting" through the floor
        if vel.y < -4.0 {
            vel.y = -4.0;
        }
    });
}

fn poison_aura_system(grid: Res<SpatialGrid>, query: Query<(&Position, &Faction, &mut Health)>) {
    const POISON_RANGE_SQ: f32 = 25.0;
    const POISON_DAMAGE: f32 = 0.05;

    query.iter().for_each(|(pos, faction, mut health)| {
        let (cx, cy) = grid.pos_to_cell(&pos);

        // Check the 3x3 grid area around the entity
        for x in (cx - 1)..=(cx + 1) {
            for y in (cy - 1)..=(cy + 1) {
                if let Some(others) = grid.cells.get(&(x, y)) {
                    for (_other_ent, other_pos, other_faction) in others {
                        // Only damage different factions
                        if faction.0 != *other_faction {
                            let dx = pos.x - other_pos.x;
                            let dy = pos.y - other_pos.y;
                            let dist_sq = dx * dx + dy * dy;

                            if dist_sq < POISON_RANGE_SQ {
                                health.0 -= POISON_DAMAGE;
                            }
                        }
                    }
                }
            }
        }
    });
}

fn sprite_transform_system(query: Query<(&Position, &mut Sprite)>) {
    query.iter().for_each(|(pos, mut sprite)| {
        // In a real Bedrock server, you'd check if the position
        // changed enough to warrant a network packet (interpolation)
        // Here we just sync the IDs for the renderer/packet-builder

        // Example: If an entity falls below the world, hide the sprite
        if pos.y < -64.0 {
            sprite.visible = false;
        } else {
            sprite.visible = true;
        }

        // Simulating a sprite index update based on X-direction (flipping)
        sprite.id = if pos.x > 0.0 { 1 } else { 0 };
    });
}

// fn ui_health_bar_system(query: Query<(&Health, &Position)>) {
//     // We iterate read-only. This system can run in parallel
//     // with any other read-only system (like a Renderer).
//     for (health, pos) in &query {
//         // Stress test: formatting strings is surprisingly expensive
//         // compared to raw math. This simulates generating hover-text.
//         if health.0 < 50.0 {
//             // let _debug_label = format!("HP: {:.1} at ({:.1}, {:.1})", health.0, pos.x, pos.y);
//             // In your actual server, you would push this to a
//             // "NetworkEvents" resource or a broadcast buffer.
//         }
//     }
// }

fn sync_point(world: &World) {}

#[inline(never)]
fn update_spatial_grid_system(
    query: Query<(Entity, &Position, &Faction)>,
    mut grid: ResMut<SpatialGrid>,
) {
    grid.cells.clear();
    for (entity, pos, faction) in &query {
        let cell = grid.pos_to_cell(&pos);
        grid.cells
            .entry(cell)
            .or_default()
            .push((entity, *pos, faction.0));
    }
}

#[test]
fn massive_world_stress_test() {
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let fmt = tracing_subscriber::fmt::Layer::new()
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .compact()
        .with_filter(
            tracing_subscriber::filter::Targets::new()
                .with_target("bedrock_ecs", tracing::Level::TRACE),
        );

    tracing_subscriber::registry().with(fmt).init();

    rayon::ThreadPoolBuilder::new()
        .num_threads(6)
        .build_global()
        .unwrap();

    let mut world = World::new();
    let mut rng = rand::rng();

    // 1. Initial Load: 10,000 Entities
    // This pushes the ECS beyond L3 cache limits (~2-5MB of component data)
    tracing::info!("Summoning entities");
    for i in 0..10_000 {
        world.spawn((
            Position {
                x: rng.random(),
                y: rng.random(),
            },
            Velocity {
                x: rng.random(),
                y: rng.random(),
            },
            Health(rng.random_range(10.0..100.0)),
            Faction((i % 4) as u8),
            Stamina(100.0),
            Sprite {
                id: i,
                visible: true,
            },
            Target(None),
        ));
    }

    world.add_resources(SpatialGrid::new(1.0));

    let mut schedule_builder = ScheduleBuilder::new(&mut world)
        .add(Physics, (physics_step_system, gravity_apply_system))
        .add(Logic, (ai_perception_system, chaos_spawner_system))
        .add(
            Combat,
            (
                vitality_system,
                poison_aura_system,
                update_spatial_grid_system,
                sync_point,
            ),
        )
        .add(Visuals, (sprite_transform_system));

    let status = std::process::Command::new("cargo")
        .args([
            "build",
            "-p",
            "test-plugin",
            "--release",
            "--target",
            "wasm32-wasip2",
        ])
        .status()
        .expect("failed to build test plugin");

    assert!(status.success());

    const WASM_PATH: &str = "../../target/wasm32-wasip2/release/test_plugin.wasm";
    // const WASM_PATH: &str = "target/wasm32-wasip2/release/test_plugin.wasm";

    let mut plugins = PluginRegistry::new().unwrap();
    plugins.add(WASM_PATH).unwrap();

    plugins.resolve_systems(&mut schedule_builder).unwrap();

    let mut schedule = schedule_builder.schedule();
    let schedule_render = schedule.render_dependency_graph();

    std::fs::write("schedule.html", schedule_render)
        .expect("failed to write schedule render to file");

    // 2. Benchmarking the Tick
    let start = std::time::Instant::now();
    let ticks = 50;
    // let ticks = 1;

    for i in 0..ticks {
        schedule.run(&world);
        // CRITICAL: Apply commands to trigger the archetype migrations
        world.apply_commands();

        if i == 0 {
            let execution_render = schedule.render_execution_graph();
            std::fs::write("execution.html", execution_render)
                .expect("failed to write execution render to file");
        }
    }

    let duration = start.elapsed();
    println!(
        "\n--- STRESS TEST RESULTS ---\n\
        Total Entities: {}\n\
        Total Ticks: {}\n\
        Avg Tick Time: {:?}\n\
        Est. TPS: {:.2}\n",
        world.alive_count(),
        ticks,
        duration / ticks as u32,
        ticks as f64 / duration.as_secs_f64()
    );

    // // 3. Validation
    // // Ensure data actually changed
    // let query = world.query::<&Position>();
    // let first_pos = query.iter().next().unwrap();
    // assert!(
    //     first_pos.x != 0.0 || first_pos.y != 0.0,
    //     "Entities did not move!"
    // );
}

// #[test]
// fn plugin_test() {
//     let status = std::process::Command::new("cargo")
//         .args(["build", "-p", "test-plugin", "--target", "wasm32-wasip2"])
//         .status()
//         .expect("failed to build test plugin");

//     assert!(status.success());

//     const WASM_PATH: &str = "../../target/wasm32-wasip2/debug/test_plugin.wasm";

//     let mut plugins = PluginRegistry::new().unwrap();
//     plugins.add(WASM_PATH).unwrap();
// }
