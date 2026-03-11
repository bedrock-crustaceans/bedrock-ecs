use std::{alloc::Layout, any::TypeId, cell::UnsafeCell, collections::{HashMap, hash_map}, iter::FusedIterator, marker::PhantomData, ptr::NonNull, sync::atomic::Ordering};

use futures::stream::FilterMap;

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;
use crate::{component::{Component, ComponentId}, entity::{EntityId, EntityMeta}, query::QueryBundle, spawn::SpawnBundle, table::Table, util::{self}};

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

pub struct ArchetypeIter<'a, T> {
    table_iter: hash_map::Iter<'a, ArchetypeComponents, Table>,
    curr_table: &'a Table,
    _marker: PhantomData<&'a T>

    // /// Which archetype table we are currently iterating over
    // table_index: usize,
    // /// The index within the current archetype table.
    // col_index: usize,
    // archetypes: &'a Archetypes,
    // _marker: PhantomData<&'a T>
}

impl<'a, T> ArchetypeIter<'a, T> {
    pub fn new(archetypes: &'a Archetypes) -> Option<ArchetypeIter<'a, T>> {
        let mut table_iter = archetypes
            .tables
            .iter();

        let curr_table = table_iter.next()?.1;        
        Some(ArchetypeIter {
            table_iter, curr_table,  _marker: PhantomData
        })
    }
}

impl<'a, T> Iterator for ArchetypeIter<'a, T> {
    type Item = NonNull<T>;

    fn next(&mut self) -> Option<NonNull<T>> {
        todo!()
    }
}

#[derive(Default, Debug)]
pub struct Archetypes {
    tables: HashMap<ArchetypeComponents, Table>,
    // lookup: HashMap<ArchetypeComponents, ArchetypeId>
}

impl Archetypes {
    pub fn new() -> Archetypes {
        Archetypes::default()
    }

    pub fn insert<B: SpawnBundle + 'static>(&mut self, id: EntityId, bundle: B) {
        let comps = B::components();
    
        let table = self.tables.entry(comps.clone())
            .or_insert_with(|| {
                Table::new::<B>()
            });

        table.insert(id, bundle);
    }

    pub fn query<B: QueryBundle>(&self) -> Option<B::Iter<'_>> {
        // self.tables.get(id)
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