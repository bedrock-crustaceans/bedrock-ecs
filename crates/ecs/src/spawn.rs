use std::{alloc::Layout, collections::HashMap};

use crate::{archetype::{ArchetypeComponents, Archetypes}, component::{Component, ComponentId}, entity::EntityId, table::Column};

pub unsafe trait SpawnGroup: 'static {
    /// Returns a list of components in this group.
    fn components() -> ArchetypeComponents;
    /// Creates a new table map to store in the archetype.
    fn new_table_map() -> HashMap<ComponentId, Column>;
    /// Inserts into an existing archetype.
    fn insert_into(self, storage: &mut HashMap<ComponentId, Column>);
}

unsafe impl SpawnGroup for () {
    fn components() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([]))
    }

    fn new_table_map() -> HashMap<ComponentId, Column> {
        HashMap::new()
    }

    fn insert_into(self, _storage: &mut HashMap<ComponentId, Column>) {
        // No-op
    }
}

unsafe impl<C: Component> SpawnGroup for C {
    fn components() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<C>()]))
    }

    fn new_table_map() -> HashMap<ComponentId, Column> {
        let id = ComponentId::of::<C>();
        let table = Column::new::<C>();

        HashMap::from([(id, table)])
    }

    fn insert_into(self, storage: &mut HashMap<ComponentId, Column>) {
        let id = ComponentId::of::<C>();

        storage.get_mut(&id).expect("ComponentBundle insertion failed").push(self);
    }
}

unsafe impl<C1: Component, C2: Component> SpawnGroup for (C1, C2) {
    fn components() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<C1>(), ComponentId::of::<C2>()]))
    }

    fn new_table_map() -> HashMap<ComponentId, Column> {
        let id1 = ComponentId::of::<C1>();
        let id2 = ComponentId::of::<C2>();

        let table1 = Column::new::<C1>();
        let table2 = Column::new::<C2>();

        HashMap::from([
            (id1, table1), 
            (id2, table2)
        ])
    }

    fn insert_into(self, storage: &mut HashMap<ComponentId, Column>) {
        let id1 = ComponentId::of::<C1>();
        let id2 = ComponentId::of::<C2>();

        storage.get_mut(&id1).expect("ComponentBundle insertion failed").push(self.0);
        storage.get_mut(&id2).expect("ComponentBundle insertion failed").push(self.1);
    }
}