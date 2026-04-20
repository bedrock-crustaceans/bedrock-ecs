use std::any::TypeId;
use std::marker::PhantomData;
#[cfg(feature = "plugins")]
use std::sync::Arc;
#[cfg(feature = "plugins")]
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use rustc_hash::FxHashMap;

use crate::plugins::WasmSystem;
use crate::scheduler::{AccessDesc, AccessType, ScheduleGraph, Scheduler};
use crate::system::{IntoSys, Sys, SysArgGroup, SysContainer, SysId, SysMeta, TypedSys};
use crate::world::World;

pub struct Chained<P: SysArgGroup, G: SystemGroup<P>> {
    group: G,
    _marker: PhantomData<P>,
}

// impl<P: SysArgGroup, G: IntoSys<P> + 'static> IntoSys<P> for Chained<P, G> {
//     fn into_boxed_sys(self, world: &mut World) -> Box<dyn Sys> {
//         self.group.into_boxed_sys(world)
//     }

//     fn into_sys(self, world: &mut World) -> impl Sys + 'static {
//         self.group.
//     }
// }

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid system group",
    label = "not a valid system group",
    note = "ensure that each of the items in the tuple is a valid system",
    note = "only tuples with up to 10 elements can be used as groups"
)]
pub trait SystemGroup<P> {
    /// Inserts this bundle into the schedule builder.
    fn insert_into(self, schedule: &mut ScheduleBuilder);

    // fn chain<P1: SysArgGroup, G: SystemGroup<P1>>(self, after: G) -> Chained<P1, G>;
}

impl<P: SysArgGroup, S: IntoSys<P>> SystemGroup<P> for S {
    fn insert_into(self, schedule: &mut ScheduleBuilder) {
        schedule.systems.push(self.into_boxed_sys(schedule.world));
    }
}

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens, non_snake_case)]
            #[diagnostic::do_not_recommend]
            impl<$([<$gen Func>]),*, $($gen),*> SystemGroup<($($gen),*)> for ($([<$gen Func>]),*)
            where
                $([<$gen Func>]: SystemGroup<$gen>),*
            {
                fn insert_into(self, schedule: &mut ScheduleBuilder) {
                    let ($($gen),*) = self;
                    $(
                        $gen.insert_into(schedule);
                    )*
                }
            }
        }
    }
}

// impl_bundle!(A);
impl_bundle!(A, B);
impl_bundle!(A, B, C);
impl_bundle!(A, B, C, D);
impl_bundle!(A, B, C, D, E);
impl_bundle!(A, B, C, D, E, F);
impl_bundle!(A, B, C, D, E, F, G);
impl_bundle!(A, B, C, D, E, F, G, H);
impl_bundle!(A, B, C, D, E, F, G, H, I);
impl_bundle!(A, B, C, D, E, F, G, H, I, J);

pub trait ScheduleLabel {
    const NAME: &'static str;
}

pub struct SystemDescriptor<'a> {
    name: &'a str,
    access: &'a [AccessDesc],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemLabelId(pub(crate) TypeId);

pub struct ScheduleBuilder<'w> {
    pub(crate) next_id: Arc<AtomicU32>,

    pub(crate) world: &'w mut World,
    pub(crate) systems: Vec<Box<dyn Sys>>,
}

impl<'w> ScheduleBuilder<'w> {
    pub fn new(world: &'w mut World) -> ScheduleBuilder<'w> {
        ScheduleBuilder {
            next_id: Arc::new(AtomicU32::new(0)),
            world,
            systems: Vec::new(),
        }
    }

    #[must_use = "dropping the builder without calling `ScheduleBuilder::schedule` will do nothing"]
    pub fn add<L, G, P>(mut self, _label: L, systems: G) -> ScheduleBuilder<'w>
    where
        L: ScheduleLabel,
        G: SystemGroup<P>,
    {
        systems.insert_into(&mut self);

        self
    }

    pub(crate) fn add_contained(&mut self, system: Box<dyn Sys>) {
        self.systems.push(system);
    }

    #[must_use = "the resulting scheduler must be used to run the schedule"]
    pub fn schedule(self) -> Scheduler {
        // Build the dependency graph
        let mut graph = ScheduleGraph::new();

        let mut writers = FxHashMap::<AccessType, usize>::default();
        let mut readers = FxHashMap::<AccessType, Vec<usize>>::default();

        graph.edges.resize_with(self.systems.len(), Vec::new);
        for (i, sys) in self.systems.iter().enumerate() {
            let access = sys.access();
            for desc in access {
                // If there is an active world system, create an edge
                if let Some(&prev_writer) = writers.get(&AccessType::World) {
                    graph.edges[prev_writer].push(i);
                }

                if desc.ty == AccessType::World {
                    // Create edge with every current reader and writer
                    for writer in writers.values() {
                        graph.edges[*writer].push(i);
                    }

                    for vec in readers.values() {
                        for reader in vec {
                            graph.edges[*reader].push(i);
                        }
                    }

                    writers.clear();
                    readers.clear();
                    writers.insert(desc.ty, i);

                    continue;
                }

                if desc.mutable {
                    // If there exist writers or readers, create an edge
                    if let Some(prev_writer) = writers.insert(desc.ty, i) {
                        graph.edges[prev_writer].push(i);
                    }

                    if let Some(prev_readers) = readers.get_mut(&desc.ty) {
                        for reader in prev_readers.iter() {
                            graph.edges[*reader].push(i);
                        }
                        prev_readers.clear();
                    }
                } else {
                    if let Some(&prev_writer) = writers.get(&desc.ty) {
                        graph.edges[prev_writer].push(i);
                    }

                    readers.entry(desc.ty).or_insert_with(Vec::new).push(i);
                }
            }
        }

        graph.edges.iter_mut().for_each(|v| v.dedup());

        // TODO: We should perform transitive reduction to remove redundant edges.

        let mut scheduler = Scheduler {
            curr_in_degrees: Vec::with_capacity(self.systems.len()),
            in_degrees: Vec::with_capacity(self.systems.len()),
            systems: self.systems,
            graph,
            ..Default::default()
        };

        scheduler.build_static_in_degrees();
        scheduler.reset_in_degrees();

        println!("{:?} {:?}", scheduler.in_degrees, scheduler.graph);

        scheduler
    }
}
