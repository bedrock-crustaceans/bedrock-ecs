use std::any::TypeId;
use std::collections::HashMap;
use crate::graph::{GraphNode, Schedule, ScheduleGraph};
use crate::param::ParamBundle;
use crate::system::{IntoSystem, System, SystemId};
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
                        let boxed = [<$gen:lower>].into_system(schedule.world);
                        let sid = SystemId::of::<$gen, [<$gen Fun>]>();
                        schedule.graph.add_node(GraphNode {
                            sid, access: boxed.access().into()
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
    pub(crate) graph: ScheduleGraph,
    pub(crate) systems: HashMap<SystemId, Box<dyn System>>,
}

impl<'w> ScheduleBuilder<'w> {
    pub fn new(world: &'w mut World) -> ScheduleBuilder<'w> {
        ScheduleBuilder {
            world,
            graph: ScheduleGraph::new(),
            systems: HashMap::new()
        }
    }

    pub fn add<L, G, P>(mut self, label: L, systems: G) -> ScheduleBuilder<'w>
    where
        L: ScheduleLabel, G: SystemBundle<P>
    {
        systems.insert_into(&mut self);

        self
    }

    pub fn schedule(mut self) -> Schedule {
        println!("Rendered: {}", self.graph.render(&self.systems));
        self.graph.sort(self.systems)
    }
}