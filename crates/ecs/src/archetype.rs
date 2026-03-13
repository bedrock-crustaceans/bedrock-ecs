use std::{alloc::Layout, any::TypeId, cell::UnsafeCell, collections::{HashMap, hash_map}, iter::FusedIterator, marker::PhantomData, ptr::NonNull, sync::atomic::Ordering};

use smallvec::SmallVec;

#[cfg(debug_assertions)]
use crate::util::debug::RwFlag;
use crate::{bitset::BitSet, component::{Component, ComponentId, ComponentRegistry}, entity::{EntityId, EntityMeta}, query::{CachedTable, QueryBundle}, spawn::SpawnBundle, table::Table, util::{self}};
use crate::filter::FilterBundle;

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
    pub(crate) registry: ComponentRegistry,
    tables: Vec<Table>,
    /// Maps archetypes to indices in the `tables` vec.
    lookup: HashMap<BitSet, usize>
}

impl Archetypes {
    pub fn new() -> Archetypes {
        Archetypes::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg_attr(
        feature = "tracing", 
        tracing::instrument(name = "Archetypes::cache_tables", skip_all)
    )]
    pub fn cache_tables<Q: QueryBundle, F: FilterBundle>(
        &self, archetype: &BitSet, filter: &F, 
        
        #[cfg(feature = "generics")]
        cache: &mut SmallVec<[CachedTable<Q::AccessCount>; 8]>,
        #[cfg(not(feature = "generics"))]
        cache: &mut SmallVec<[CachedTable; 8]>
    ) {
        cache.clear();

        let iter = self.lookup.iter().enumerate().filter_map(|(i, (k, &v))| {
            if k.is_subset(archetype) {
                // This table matches the queried components. We now apply all passive filters.
                // Dynamic filters will be applied during iteration. 
                if !filter.apply_static_filters(k) {
                    return None
                }

                // Found match
                let table = &self.tables[v];
                let cols = Q::cache_layout(&table.lookup);
                return Some(CachedTable {
                    table: v,
                    cols
                })
            }            

            None
        });

        cache.extend(iter);
    }

    #[cfg_attr(
        feature = "tracing", 
        tracing::instrument(name = "Archetypes::insert", skip(self, bundle))
    )]
    pub fn insert<B: SpawnBundle + 'static>(&mut self, id: EntityId, bundle: B) {
        self.generation += 1;
        
        let comps = B::components(&mut self.registry);

        // Check whether archetype already exists, otherwise create it
        let lookup = self.lookup.get(&comps);
        let table = if let Some(index) = lookup {
            &mut self.tables[*index]
        } else {
            self.lookup.insert(comps.clone(), self.tables.len());
            let table = Table::new::<B>(comps);
            self.tables.push(table);

            self.tables.last_mut().unwrap()
        };

        table.insert(id, bundle);
    }

    pub fn table(&self, index: usize) -> &Table {
        &self.tables[index]
    }
    
    pub fn remove(&mut self, id: &ArchetypeComponents) -> Option<Table> {
        self.generation += 1;
        todo!()
        // self.tables.remove(id)
    }
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

        println!("Dropping archetypes");
    }
}