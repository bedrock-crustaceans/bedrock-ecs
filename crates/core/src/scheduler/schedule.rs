use std::any::TypeId;

use rustc_hash::FxHashMap;

use crate::scheduler::{AccessType, ScheduleGraph, ScheduleNode, Scheduler};
use crate::system::{IntoSystem, ParamBundle, System, SystemId};
use crate::world::World;

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid system bundle",
    label = "not a valid system bundle",
    note = "ensure that each of the items in the tuple is a valid system",
    note = "only tuples with up to 10 elements can be used as bundles"
)]
pub trait SystemBundle<P> {
    /// Inserts this bundle into the schedule builder.
    fn insert_into(self, schedule: &mut ScheduleBuilder);
}

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            #[diagnostic::do_not_recommend]
            impl<$([<$gen Fun>]),*, $($gen),*> SystemBundle<($($gen),*)> for ($([<$gen Fun>]),*)
            where
                $([<$gen Fun>]: IntoSystem<$gen> + 'static),*,
                $($gen: ParamBundle),*
            {
                fn insert_into(self, schedule: &mut ScheduleBuilder) {
                    let ($([<$gen:lower>]),*) = self;
                    $(
                        let sid = schedule.next_id();
                        let boxed = [<$gen:lower>].into_system(schedule.world, sid);

                        schedule.systems.push(boxed);
                    )*
                }
            }
        }
    }
}

impl_bundle!(A);
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemLabelId(pub(crate) TypeId);

pub struct ScheduleBuilder<'w> {
    next_id: u32,
    pub(crate) world: &'w mut World,
    pub(crate) systems: Vec<Box<dyn System>>,
}

impl<'w> ScheduleBuilder<'w> {
    pub fn new(world: &'w mut World) -> ScheduleBuilder<'w> {
        ScheduleBuilder {
            next_id: 0,
            world,
            systems: Vec::new(),
        }
    }

    pub(crate) fn next_id(&mut self) -> SystemId {
        let id = self.next_id;
        self.next_id += 1;
        SystemId(id)
    }

    #[must_use = "dropping the builder without calling `ScheduleBuilder::schedule` will do nothing"]
    pub fn add<L, G, P>(mut self, _label: L, systems: G) -> ScheduleBuilder<'w>
    where
        L: ScheduleLabel,
        G: SystemBundle<P>,
    {
        systems.insert_into(&mut self);

        self
    }

    pub fn schedule(self) -> Scheduler {
        // Build the dependency graph
        let mut graph = ScheduleGraph::new();

        let mut writers = FxHashMap::<AccessType, usize>::default();
        let mut readers = FxHashMap::<AccessType, Vec<usize>>::default();

        graph.edges.resize_with(self.systems.len(), Vec::new);
        for (i, sys) in self.systems.iter().enumerate() {
            let access = sys.access();
            for desc in access {
                if desc.exclusive {
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
        };

        scheduler.build_static_in_degrees();
        scheduler.reset_in_degrees();

        println!("{:?} {:?}", scheduler.in_degrees, scheduler.graph);

        scheduler
    }
}
