use std::any::TypeId;
use std::collections::HashMap;
use crate::graph::{GraphNode, Schedule, ScheduleGraph};
use crate::param::ParamBundle;
use crate::system::{IntoSystem, System, SystemId};

pub trait SystemBundle<P> {
    fn insert_into(self, schedule: &mut ScheduleBuilder);
}

impl<F, P> SystemBundle<P> for F
where
    P: ParamBundle,
    F: IntoSystem<P> + 'static
{
    fn insert_into(self, schedule: &mut ScheduleBuilder) {
        let sid = SystemId::of::<P, F>();
        let boxed = self.into_system();

        schedule.graph.add_node(GraphNode {
            sid, access: boxed.access()
        });
        schedule.systems.insert(sid, boxed);
    }
}

impl<F1, F2, P1, P2> SystemBundle<(P1, P2)> for (F1, F2)
where
    P1: ParamBundle,
    P2: ParamBundle,
    F1: IntoSystem<P1> + 'static,
    F2: IntoSystem<P2> + 'static,
{
    fn insert_into(self, schedule: &mut ScheduleBuilder) {
        let sid1 = SystemId::of::<P1, F1>();
        let sid2 = SystemId::of::<P2, F2>();
        let boxed1 = self.0.into_system();
        let boxed2 = self.1.into_system();

        schedule.graph.add_node(GraphNode {
            sid: sid1, access: boxed1.access()
        });

        schedule.graph.add_node(GraphNode {
            sid: sid2, access: boxed2.access()
        });

        schedule.systems.insert(sid1, boxed1);
        schedule.systems.insert(sid2, boxed2);
    }
}

pub trait SystemsLabel {
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
        L: SystemsLabel, G: SystemBundle<P>
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