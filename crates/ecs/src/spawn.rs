use std::alloc::Layout;

use crate::{archetype::{ArchetypeComponents, Archetypes}, component::{Component, ComponentId, Components}, entity::EntityId};

pub unsafe trait ComponentGroup {
    fn layout() -> Layout;
    fn archetype() -> ArchetypeComponents;
    fn insert_into(self, entity: EntityId, archs: &mut Archetypes);
}

unsafe impl<C: Component> ComponentGroup for C {
    fn layout() -> Layout {
        Layout::new::<C>()
    }

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<C>()]))
    }

    fn insert_into(self, entity: EntityId, archs: &mut Archetypes) {
        archs.insert(entity, self);
    }
}

unsafe impl<C1: Component, C2: Component> ComponentGroup for (C1, C2) {
    fn layout() -> Layout {
        Layout::new::<(C1, C2)>()
    }

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<C1>(), ComponentId::of::<C2>()]))
    }

    fn insert_into(self, entity: EntityId, archs: &mut Archetypes) {
        let comps = ArchetypeComponents(Box::new([
            ComponentId::of::<C1>(), ComponentId::of::<C2>()
        ]));

        archs.insert(entity, self);
    }
}