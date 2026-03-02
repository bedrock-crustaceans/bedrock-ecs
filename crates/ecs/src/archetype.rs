use std::{alloc::Layout, any::TypeId, cell::UnsafeCell, collections::HashMap, iter::FusedIterator, marker::PhantomData, ptr::NonNull, sync::atomic::Ordering};

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;
use crate::{component::{Component, ComponentId}, entity::{EntityId, EntityMeta}, spawn::ComponentBundle, table::Table, util::{self}};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ArchetypeId(pub(crate) usize);

impl From<usize> for ArchetypeId {
    fn from(v: usize) -> ArchetypeId {
        ArchetypeId(v)
    }
}

/// A list of components contained in an archetype.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchetypeComponents(pub(crate) Box<[ComponentId]>);

#[derive(Default, Debug)]
pub struct Archetypes {
    tables: HashMap<ArchetypeComponents, Table>,
    // lookup: HashMap<ArchetypeComponents, ArchetypeId>
}

impl Archetypes {
    pub fn new() -> Archetypes {
        Archetypes::default()
    }

    pub fn insert<G: ComponentBundle + 'static>(&mut self, id: EntityId, bundle: G) {
        let comps = G::components();
    
        let table = self.tables.entry(comps.clone())
            .or_insert_with(|| {
                Table::new::<G>()
            });

        table.insert(id, bundle);
    }

    pub fn get(&self, id: &ArchetypeComponents) -> Option<&Table> {
        self.tables.get(id)
    }
    
    pub fn remove(&mut self, id: &ArchetypeComponents) -> Option<Table> {
        self.tables.remove(id)
    }

    // pub fn insert(&mut self, id: EntityId, components: ArchetypeComponents, layout: Layout) {
    //     let idx = self.lookup.get(&components).copied().unwrap_or_else(|| {
    //         let archetype = Archetype::new(components.clone(), layout);
    //         self.archetypes.push(archetype);
            
    //         let id = ArchetypeId::from(self.archetypes.len() - 1);
    //         self.lookup.insert(components, id);

    //         id
    //     });

    //     let archetype = &mut self.archetypes[idx.0];
    //     todo!();
    // }
}

#[cfg(test)]
mod test {
    use crate::{archetype::Archetypes, component::Component, entity::EntityId};

    struct Test {
        hello: usize
    }

    impl Component for Test {}

    impl Drop for Test {
        fn drop(&mut self) {
            println!("Test {} has been dropped", self.hello);
        }
    }

    #[test]
    fn create_archetype() {
        let mut archetypes = Archetypes::new();
        
        archetypes.insert(EntityId(0), Test { hello: 0 });
        archetypes.insert(EntityId(1), Test { hello: 1 });
        archetypes.insert(EntityId(2), Test { hello: 2 });
        

        // println!("{archetypes:?}");
        println!("Dropping archetypes");
    }
}