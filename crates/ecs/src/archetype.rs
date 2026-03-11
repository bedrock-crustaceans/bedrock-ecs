use std::{alloc::Layout, any::TypeId, cell::UnsafeCell, collections::{HashMap, hash_map}, iter::FusedIterator, marker::PhantomData, ptr::NonNull, sync::atomic::Ordering};

use futures::stream::FilterMap;
use smallvec::SmallVec;

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;
use crate::{component::{Component, ComponentId}, entity::{EntityId, EntityMeta}, query::{CachedTable, QueryBundle}, spawn::SpawnBundle, table::Table, util::{self}};

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

impl ArchetypeComponents {
    /// Whether `other` is a subset of `self`.
    pub fn is_subset(&self, other: &ArchetypeComponents) -> bool {
        other.0.iter().all(|o| self.0.contains(o))
    }
}

pub struct ArchetypeIter<'a, T: QueryBundle> {
    /// The current table being iterated over.
    table: &'a Table,
    /// The current index
    index: usize,
    _marker: PhantomData<&'a T>
}

impl<'a, T: QueryBundle> ArchetypeIter<'a, T> {
    pub fn new(table: &'a Table) -> ArchetypeIter<'a, T> {
        ArchetypeIter {
            table, 
            index: 0, 
            _marker: PhantomData
        }
    }
}

impl<'a, T: QueryBundle> Iterator for ArchetypeIter<'a, T> {
    type Item = T::Output<'a>;

    fn next(&mut self) -> Option<T::Output<'a>> {
        todo!()
    }
}

#[derive(Default, Debug)]
pub struct Archetypes {
    generation: u64,
    tables: Vec<Table>,
    lookup: HashMap<Box<[TypeId]>, usize>
}

impl Archetypes {
    pub fn new() -> Archetypes {
        Archetypes::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn cache_tables<T: QueryBundle>(&self,  cache: &mut SmallVec<[CachedTable; 8]>) {
        cache.clear();

        let iter = self.lookup.keys().enumerate().filter_map(|(i, k)| {
            let cols = T::cache_layout(k);

            // This table contains requested components, cache it.
            (!cols.is_empty()).then_some(CachedTable {
                table: i, cols
            })
        });

        cache.extend(iter);
    }

    pub fn insert<B: SpawnBundle + 'static>(&mut self, id: EntityId, bundle: B) {
        self.generation += 1;
        
        let comps = B::components();

        // Check whether archetype already exists, otherwise create it
        let lookup = self.lookup.get(&comps);
        let table = if let Some(index) = lookup {
            &mut self.tables[*index]
        } else {
            self.lookup.insert(comps, self.tables.len());
            let table = Table::new::<B>();
            self.tables.push(table);

            self.tables.last_mut().unwrap()
        };

        table.insert(id, bundle);
    }

    pub fn query<B: QueryBundle>(&self) -> Option<B::Iter<'_>> {
        todo!()
        // self.tables.get(id)
    }
    
    pub fn remove(&mut self, id: &ArchetypeComponents) -> Option<Table> {
        self.generation += 1;
        todo!()
        // self.tables.remove(id)
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