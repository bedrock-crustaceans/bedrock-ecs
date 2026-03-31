use std::{
    hash::{Hash, Hasher},
    sync::{
        Condvar,
        atomic::{AtomicU32, AtomicUsize, Ordering},
        mpsc,
    },
    u32,
};

use rayon::Scope;
use rustc_hash::{FxHashMap, FxHasher};

use crate::{
    scheduler::ScheduleGraph,
    system::{System, SystemId},
    world::World,
};

fn hash_system_id(id: SystemId) -> u64 {
    let mut hasher = FxHasher::with_seed(0);
    id.hash(&mut hasher);
    hasher.finish()
}

pub struct Scheduler {
    pub(crate) systems: Vec<Box<dyn System>>,
    pub(crate) in_degrees: Vec<u32>,
    pub(crate) curr_in_degrees: Vec<AtomicU32>,
    pub(crate) graph: ScheduleGraph,
}

impl Scheduler {
    pub(crate) fn build_static_in_degrees(&mut self) {
        self.in_degrees.resize(self.systems.len(), 0);
        for from in &self.graph.edges {
            for to in from {
                self.in_degrees[*to] += 1;
            }
        }
    }

    pub(crate) fn reset_in_degrees(&mut self) {
        self.curr_in_degrees
            .resize_with(self.in_degrees.len(), || AtomicU32::new(u32::MAX));

        self.curr_in_degrees
            .iter()
            .zip(self.in_degrees.iter())
            .for_each(|(a, b)| a.store(*b, Ordering::Release));
    }

    fn run_system<'a>(&'a self, id: usize, world: &'a World, s: &Scope<'a>) {
        s.spawn(move |s| {
            // Ensure the current system does not run again.
            self.curr_in_degrees[id].store(u32::MAX, Ordering::Release);

            let system = &self.systems[id];
            tracing::trace!("calling {}", system.meta().name());
            unsafe { system.call(world) };
            tracing::trace!("finished {}", system.meta().name());

            tracing::error!("{:?}", self.curr_in_degrees);
            // Then decrease all in degrees of the dependent systems
            for dependent in &self.graph.edges[id] {
                let now = self.curr_in_degrees[*dependent].fetch_sub(1, Ordering::Relaxed) - 1;
                if now == 0 {
                    // This system has no more dependencies, it can run immediately
                    self.run_system(*dependent, world, s);
                }
            }
        });
    }

    pub fn run(&mut self, world: &World) {
        tracing::error!("schedule is: {:?}", self.in_degrees);

        rayon::scope(|s| {
            for id in self
                .curr_in_degrees
                .iter()
                .enumerate()
                .filter_map(|(i, d)| (d.load(Ordering::Acquire) == 0).then_some(i))
            {
                tracing::trace!("running {id}");
                self.run_system(id, world, s);
            }
        });
    }

    /// Returns HTML of a graph of the schedule.
    pub fn render(&self) -> String {
        let mut nodes = String::new();
        for (i, sys) in self.systems.iter().enumerate() {
            let name = sys.meta().name();

            nodes += &format!("{i}(\"{name}\");");
        }

        let mut edges = String::new();
        for (from, to_vec) in self.graph.edges.iter().enumerate() {
            for to in to_vec {
                edges += &format!("{from} --> {to};");
            }
        }

        format!(
            r#"
                <!DOCTYPE html>
                <html>
                    <head>
                        <title>Bedrock ECS scheduler graph</title>
                    </head>
                    <body>
                        <script type="module">
                            import mermaid from "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs";
                            mermaid.initialize({{ startOnLoad: true }});
                        </script>

                        <pre class="mermaid">
                            ---
                            title: Bedrock ECS Scheduler Graph
                            config:
                                theme: neutral
                                look: handDrawn
                            ---
                            flowchart TD
                                {nodes}
                                {edges}
                        </pre>
                    </body>
                </html>
            "#
        )
    }
}
