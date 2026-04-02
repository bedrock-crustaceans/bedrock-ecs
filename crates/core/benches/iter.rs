use bedrock_ecs::{query::Query, world::World};
use bedrock_ecs_derive::Component;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};



#[derive(Component, bevy_ecs::component::Component)]
struct Comp {
    data: [f32; 3],
}

fn bench_system(query: Query<&Comp>) {
    for v in &query {
        std::hint::black_box(v);
    }
}

fn bevy_bench_system(query: bevy_ecs::prelude::Query<&Comp>) {
    for v in &query {
        std::hint::black_box(v);
    }
}

fn iter_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter");

    for size in [10_000, 100_000, 1_000_000] {
        // group.throughput(Throughput::Elements(size));

        let mut world = World::new();
        for _ in 0..size {
            world.spawn(Comp { data: [0.0; 3] });
        }

        group.bench_function(BenchmarkId::new("bedrock", size), |b| {
            b.iter(|| world.run_system(bench_system))
        });

        let mut world = bevy_ecs::prelude::World::new();
        world.spawn_batch((0..size).map(|_| Comp { data: [0.0; 3] }));

        let id = world.register_system(bevy_bench_system);

        group.bench_function(BenchmarkId::new("bevy", size), |b| {
            b.iter(|| {
                world.run_system(id);
            })
        });
    }

    group.finish();
}

criterion_group!(benches, iter_benchmark);
criterion_main!(benches);
