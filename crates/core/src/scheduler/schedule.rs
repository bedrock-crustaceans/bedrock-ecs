use std::any::TypeId;

use rustc_hash::FxHashMap;

use crate::scheduler::{AccessType, ScheduleGraph, ScheduleNode};
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
                        let sid = SystemId::of::<$gen, [<$gen Fun>]>();
                        let boxed = [<$gen:lower>].into_system(schedule.world, sid);

                        schedule.systems.insert(sid, boxed);
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
    pub(crate) world: &'w mut World,
    pub(crate) systems: FxHashMap<SystemId, Box<dyn System>>,
}

impl<'w> ScheduleBuilder<'w> {
    pub fn new(world: &'w mut World) -> ScheduleBuilder<'w> {
        ScheduleBuilder {
            world,
            systems: FxHashMap::default(),
        }
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

    pub fn schedule(self) -> ScheduleGraph {
        // Build the dependency graph
        let mut graph = ScheduleGraph::new();

        let mut writers = FxHashMap::<AccessType, usize>::default();
        let mut readers = FxHashMap::<AccessType, Vec<usize>>::default();

        graph.nodes.reserve(self.systems.len());
        for (i, sys) in self.systems.values().enumerate() {
            graph.nodes.push(ScheduleNode {
                id: sys.meta().id(),
            });

            let access = sys.access();
            for desc in access {
                if desc.exclusive {
                    // If there exist writers or readers, create an edge
                    if let Some(prev_writer) = writers.insert(desc.ty, i) {
                        graph.edges.insert((prev_writer, i));
                    }

                    if let Some(prev_readers) = readers.get_mut(&desc.ty) {
                        for reader in prev_readers.iter() {
                            graph.edges.insert((*reader, i));
                        }
                        prev_readers.clear();
                    }
                } else {
                    if let Some(&prev_writer) = writers.get(&desc.ty) {
                        graph.edges.insert((prev_writer, i));
                    }

                    readers.entry(desc.ty).or_insert_with(Vec::new).push(i);
                }
            }
        }

        // TODO: We should perform transitive reduction to remove redundant edges.

        graph.systems = self.systems;
        graph
    }
}
