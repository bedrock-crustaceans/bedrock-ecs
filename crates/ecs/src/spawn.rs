use std::{alloc::Layout, collections::HashMap};

use crate::{archetype::{Archetype, ArchetypeComponents, Archetypes, Table}, component::{Component, ComponentId}, entity::EntityId};

pub unsafe trait ComponentBundle: 'static {
    /// Returns a list of components in this group.
    fn components() -> ArchetypeComponents;
    /// Creates a new table map to store in the archetype.
    fn new_table_map() -> HashMap<ComponentId, Table>;
    /// Inserts into an existing archetype.
    fn insert_into(self, storage: &mut HashMap<ComponentId, Table>);
}

unsafe impl<C: Component> ComponentBundle for C {
    fn components() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<C>()]))
    }

    fn new_table_map() -> HashMap<ComponentId, Table> {
        let id = ComponentId::of::<C>();
        let table = Table::new::<C>();

        HashMap::from([(id, table)])
    }

    fn insert_into(self, storage: &mut HashMap<ComponentId, Table>) {
        let id = ComponentId::of::<C>();

        storage.get_mut(&id).expect("ComponentBundle insertion failed").push(self);
    }
}

unsafe impl<C1: Component, C2: Component> ComponentBundle for (C1, C2) {
    fn components() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<C1>(), ComponentId::of::<C2>()]))
    }

    fn new_table_map() -> HashMap<ComponentId, Table> {
        let id1 = ComponentId::of::<C1>();
        let id2 = ComponentId::of::<C2>();

        let table1 = Table::new::<C1>();
        let table2 = Table::new::<C2>();

        HashMap::from([
            (id1, table1), 
            (id2, table2)
        ])
    }

    fn insert_into(self, storage: &mut HashMap<ComponentId, Table>) {
        let id1 = ComponentId::of::<C1>();
        let id2 = ComponentId::of::<C2>();

        storage.get_mut(&id1).expect("ComponentBundle insertion failed").push(self.0);
        storage.get_mut(&id2).expect("ComponentBundle insertion failed").push(self.1);
    }
}