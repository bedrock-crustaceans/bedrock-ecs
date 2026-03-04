use std::any::TypeId;
use std::collections::HashMap;
use crate::graph::{GraphNode, Schedule, ScheduleGraph};
use crate::param::ParamBundle;
use crate::system::{IntoSystem, System, SystemId};

pub trait SystemBundle<P> {
    fn insert_into(self, schedule: &mut ScheduleBuilder);
}

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        paste::paste! {
            #[allow(unused_parens)]
            impl<$([<$gen F>]: IntoSystem<$gen> + 'static),*, $($gen: ParamBundle),*> SystemBundle<($($gen),*)> for ($([<$gen F>]),*) {
                fn insert_into(self, schedule: &mut ScheduleBuilder) {
                    #[allow(non_snake_case)]
                    let ($($gen),*) = self;
                    $(
                        let boxed = $gen.into_system();
                        let sid = SystemId::of::<$gen, [<$gen F>]>();
                        schedule.graph.add_node(GraphNode {
                            sid, access: boxed.access()
                        });
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

pub trait ScheduleLabel {
    const NAME: &'static str;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemLabelId(pub(crate) TypeId);

pub struct ScheduleBuilder {
    pub(crate) graph: ScheduleGraph,
    pub(crate) systems: HashMap<SystemId, Box<dyn System>>,
}

impl ScheduleBuilder {
    pub fn new() -> ScheduleBuilder {
        ScheduleBuilder {
            graph: ScheduleGraph::new(),
            systems: HashMap::new()
        }
    }

    pub fn add<L, G, P>(mut self, label: L, systems: G) -> ScheduleBuilder
    where
        L: ScheduleLabel, G: SystemBundle<P>
    {
        systems.insert_into(&mut self);

        self
    }

    pub fn schedule(&mut self) -> Schedule {
        self.graph.sort()
    }
}

impl Default for ScheduleBuilder {
    fn default() -> ScheduleBuilder {
        ScheduleBuilder::new()
    }
}