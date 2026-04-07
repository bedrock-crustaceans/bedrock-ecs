use std::{
    hash::{Hash, Hasher},
    sync::{
        Condvar,
        atomic::{AtomicU32, AtomicUsize, Ordering},
        mpsc,
    },
    time::Instant,
    u32,
};
#[cfg(feature = "inspect")]
use std::{sync::Mutex, thread::ThreadId};

use rayon::Scope;
use rustc_hash::{FxHashMap, FxHasher};

use crate::{
    scheduler::ScheduleGraph,
    system::{Sys, SysId},
    world::World,
};

#[cfg(feature = "inspect")]
#[derive(Debug, Clone)]
pub struct ExecutionInfo {
    thread: usize,
    start: Instant,
    finish: Instant,
}

impl Default for ExecutionInfo {
    fn default() -> Self {
        Self {
            thread: usize::MAX,
            start: Instant::now(),
            finish: Instant::now(),
        }
    }
}

#[cfg(feature = "inspect")]
#[derive(Debug)]
pub struct SchedulerTiming {
    start_time: Instant,
    total_threads: usize,
    /// Registers the start and stop times of a system.
    timing: Vec<ExecutionInfo>,
}

impl Default for SchedulerTiming {
    fn default() -> Self {
        Self {
            #[cfg(miri)]
            total_threads: usize::MAX,
            #[cfg(not(miri))]
            total_threads: rayon::current_num_threads(),
            start_time: Instant::now(),
            timing: Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct Scheduler {
    pub(crate) systems: Vec<Box<dyn Sys>>,
    pub(crate) in_degrees: Vec<u32>,
    pub(crate) curr_in_degrees: Vec<AtomicU32>,
    pub(crate) graph: ScheduleGraph,

    #[cfg(feature = "inspect")]
    pub(crate) timing: Mutex<SchedulerTiming>,
}

impl Scheduler {
    pub(crate) fn build_static_in_degrees(&mut self) {
        self.in_degrees.resize(self.systems.len(), 0);
        for from in &self.graph.edges {
            for to in from {
                self.in_degrees[*to] += 1;
            }
        }

        // Systems that are no in the graph have no dependencies and automatically have an in-degrees
        // count of 0.
    }

    pub(crate) fn reset_in_degrees(&mut self) {
        self.curr_in_degrees
            .resize_with(self.in_degrees.len(), || AtomicU32::new(u32::MAX));

        self.curr_in_degrees
            .iter()
            .zip(self.in_degrees.iter())
            .for_each(|(a, b)| a.store(*b, Ordering::Release));
    }

    #[cfg(miri)]
    fn run_system<'a>(&'a self, id: usize, world: &'a World, s: &'a std::thread::Scope<'a, '_>) {
        s.spawn(move || {
            // Ensure the current system does not run again.
            self.curr_in_degrees[id].store(u32::MAX, Ordering::Release);

            let system = &self.systems[id];

            #[cfg(feature = "inspect")]
            let start = Instant::now();

            dbg!("executing system {}", system.meta().name());
            unsafe { system.call(world) };
            dbg!("finished system {}", system.meta().name());

            #[cfg(feature = "inspect")]
            {
                let mut lock = self.timing.lock().unwrap();

                let finish = Instant::now();
                lock.timing[id] = ExecutionInfo {
                    start,
                    finish,
                    thread: 0,
                };
            }

            // Then decrease all in degrees of the dependent systems
            for dependent in &self.graph.edges[id] {
                let now = self.curr_in_degrees[*dependent].fetch_sub(1, Ordering::Relaxed) - 1;
                if now == 0 {
                    // This system has no more dependencies, it can run immediately
                    std::thread::scope(|s| {
                        self.run_system(*dependent, world, s);
                    });
                }
            }
        });
    }

    #[cfg(not(miri))]
    fn run_system<'a>(&'a self, id: usize, world: &'a World, s: &Scope<'a>) {
        s.spawn(move |s| {
            // Ensure the current system does not run again.
            self.curr_in_degrees[id].store(u32::MAX, Ordering::Release);

            let system = &self.systems[id];

            #[cfg(feature = "inspect")]
            let start = Instant::now();

            tracing::error!("executing system {}", system.meta().name());
            unsafe { system.call(world) };

            #[cfg(feature = "inspect")]
            {
                let mut lock = self.timing.lock().unwrap();

                let finish = Instant::now();
                lock.timing[id] = ExecutionInfo {
                    start,
                    finish,
                    thread: rayon::current_thread_index().unwrap(),
                };
            }

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

    pub fn run(&mut self, world: &mut World) {
        // Reset the in degrees.
        self.reset_in_degrees();

        #[cfg(feature = "inspect")]
        {
            let mut lock = self.timing.lock().unwrap();

            lock.start_time = Instant::now();
            lock.timing
                .resize(self.systems.len(), ExecutionInfo::default());
        }

        #[cfg(miri)]
        std::thread::scope(|s| {
            for id in self
                .curr_in_degrees
                .iter()
                .enumerate()
                .filter_map(|(i, d)| (d.load(Ordering::Acquire) == 0).then_some(i))
            {
                self.run_system(id, world, s);
            }
        });

        #[cfg(not(miri))]
        rayon::scope(|s| {
            for id in self
                .curr_in_degrees
                .iter()
                .enumerate()
                .filter_map(|(i, d)| (d.load(Ordering::Acquire) == 0).then_some(i))
            {
                self.run_system(id, world, s);
            }
        });

        world.current_tick += 1;
    }

    #[cfg(feature = "inspect")]
    pub fn render_execution_graph(&self) -> String {
        let mut sections = String::new();
        let lock = self.timing.lock().unwrap();

        for i in 0..lock.total_threads {
            let mut curr = String::new();
            for (j, info) in lock.timing.iter().enumerate() {
                if info.thread == i {
                    let name = self.systems[j].meta().name();

                    let start_elapsed = info.start.duration_since(lock.start_time);
                    let finish_elapsed = info.finish.duration_since(lock.start_time);

                    let start_micros = start_elapsed.as_micros();
                    let finish_micros = finish_elapsed.as_micros();

                    let start_fmt = format!(
                        "{:02}:{:02}.{:03}",
                        (start_micros / 60_000),        // Minutes
                        (start_micros % 60_000) / 1000, // Seconds
                        start_micros % 1000             // Milliseconds
                    );

                    let finish_fmt = format!(
                        "{:02}:{:02}.{:03}",
                        (finish_micros / 60_000),
                        (finish_micros % 60_000) / 1000,
                        finish_micros % 1000
                    );

                    curr += &format!("    {name} :{j}, {start_fmt}, {finish_fmt}\n");
                }
            }

            if !curr.is_empty() {
                sections += &format!("section Thread {i}\n{curr}");
            }
        }

        format!(
            r#"
            <!DOCTYPE html>
            <html>
                <head>
                    <title>ECS execution chart</title>
                </head>
                <body>
                    <script type="module">
                        import mermaid from "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs";
                        mermaid.initialize({{ startOnLoad: true }});
                    </script>

                    <pre class="mermaid">
---
title: ECS execution chart
---
gantt
    title ECS execution chart
    dateFormat mm:ss.SSS
    axisFormat %M:%S.%L
    {sections}
                    </pre>
                </body>
            </html>
        "#
        )
    }

    /// Returns HTML of a graph of the schedule.
    #[cfg(feature = "inspect")]
    pub fn render_dependency_graph(&self) -> String {
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
                        <title>ECS dependency graph</title>
                    </head>
                    <body>
                        <script type="module">
                            import mermaid from "https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs";
                            mermaid.initialize({{ startOnLoad: true }});
                        </script>

                        <pre class="mermaid">
                            ---
                            title: ECS dependency graph
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
