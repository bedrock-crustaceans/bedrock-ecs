use crate::{component::Components, entity::Entities, system::Systems};

#[derive(Default)]
pub struct World {
    entities: Entities,
    pub(crate) components: Components,
    pub(crate) systems: Systems
}

impl World {
    pub fn new() -> World {
        World::default()
    }
}