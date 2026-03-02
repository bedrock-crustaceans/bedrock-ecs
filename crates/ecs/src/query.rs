use std::{any::TypeId, iter::FusedIterator, marker::PhantomData, ptr::NonNull};

use smallvec::{SmallVec, smallvec};

use crate::{archetype::{ArchetypeComponents, Archetypes}, component::{Component, ComponentId}, entity::{Entity, EntityIter}, filter::FilterGroup, param::{Param, ParamDesc, QueryDesc, QueryDescVec, QueryType}, sealed::Sealed, table::{ColumnIter, ColumnIterMut, Table}, world::World};

pub trait QueryBundle {
    type Fetchable<'w>;
    type Iter<'w>: Iterator<Item = Self::Fetchable<'w>>;

    const SEND: bool;
    const MUTABLE: bool;

    fn archetype() -> ArchetypeComponents;
    unsafe fn iter<'w>(table: &'w Table) -> Self::Iter<'w>;

    unsafe fn from_ptr<'w>(ptr: NonNull<u8>) -> Self::Fetchable<'w>;
    fn desc() -> QueryDescVec;
}

impl QueryBundle for Entity<'_> {
    type Fetchable<'a> = Entity<'a>;
    type Iter<'w> = EntityIter<'w>;

    const SEND: bool = true;
    const MUTABLE: bool = false;

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([]))
    }

    unsafe fn from_ptr<'w>(_ptr: NonNull<u8>) -> Entity<'w> {
        panic!("Cannot instantiate Entity from pointer");
    }

    unsafe fn iter<'w>(_table: &'w Table) -> Self::Iter<'w> {
        todo!()
    }

    fn desc() -> QueryDescVec {
        smallvec![QueryDesc {
            mutable: false,
            ty: QueryType::Entity
        }]
    }
}

impl<T: Component + Send> QueryBundle for &T {
    type Fetchable<'a> = &'a T;
    type Iter<'w> = ColumnIter<'w, T>;

    const SEND: bool = true;
    const MUTABLE: bool = false;

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<T>()]))
    }

    unsafe fn iter<'w>(table: &'w Table) -> ColumnIter<'w, T> {
        let id = ComponentId::of::<T>();
        let col = table.col(&id);
        ColumnIter::new(col)
    }

    unsafe fn from_ptr<'w>(ptr: NonNull<u8>) -> &'w T {
        unsafe { &*(ptr.as_ptr() as *const T) }
    }

    fn desc() -> QueryDescVec {
        smallvec![QueryDesc {
            mutable: false,
            ty: QueryType::Component(TypeId::of::<T>())
        }]
    }
}

impl<T: Component + Send> QueryBundle for &mut T {
    type Fetchable<'a> = &'a mut T;
    type Iter<'w> = ColumnIterMut<'w, T>;

    const SEND: bool = true;
    const MUTABLE: bool = true;

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<T>()]))
    }

    unsafe fn iter<'w>(table: &'w Table) -> ColumnIterMut<'w, T> {
        todo!()
    }

    unsafe fn from_ptr<'w>(ptr: NonNull<u8>) -> &'w mut T {
        unsafe { &mut *(ptr.as_ptr() as *mut T) }
    }

    fn desc() -> QueryDescVec {
        smallvec![QueryDesc {
            mutable: true,
            ty: QueryType::Component(TypeId::of::<T>())
        }]
    }
}

pub struct Query<'w, Q: QueryBundle, F: FilterGroup = ()> {
    archetypes: &'w Archetypes,
    state: &'w QueryState,
    _marker: PhantomData<(Q, F)>
}

impl<'w, Q: QueryBundle, F: FilterGroup> Query<'w, Q, F> {
    pub fn new(world: &'w World, state: &'w QueryState) -> Query<'w, Q, F> {
        println!("Query mutable? {}", Q::MUTABLE);

        Query {
            archetypes: &world.archetypes,
            state,
            _marker: PhantomData
        }
    }
}

impl<'placeholder, Q: QueryBundle, F: FilterGroup> Param for Query<'placeholder, Q, F> {
    type State = QueryState;
    type Item<'w> = Query<'w, Q, F>;

    const SEND: bool = Q::SEND;

    fn desc() -> ParamDesc {
        ParamDesc::Query(Q::desc())
    }

    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut QueryState) -> Query<'w, Q, F> {
        Query::new(world, state)
    }

    fn init() -> QueryState {
        QueryState {
            archetype: Q::archetype()
        }
    }
    
    fn destroy(_: &mut QueryState) {}
}

pub struct QueryState {
    archetype: ArchetypeComponents
}

pub struct QueryIter<'q, 'w, Q: QueryBundle, F: FilterGroup> {
    iter: Option<Q::Iter<'w>>,
    _marker: PhantomData<&'q (Q, F)>
}

impl<'q, 'w, Q: QueryBundle, F: FilterGroup> IntoIterator for &'q Query<'w, Q, F> {
    type Item = Q::Fetchable<'w>;
    type IntoIter = QueryIter<'q, 'w, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        QueryIter::from(self)
    }
}

impl<'q, 'w, Q: QueryBundle, F: FilterGroup> From<&'q Query<'w, Q, F>> for QueryIter<'q, 'w, Q, F> {
    fn from(query: &'q Query<'w, Q, F>) -> QueryIter<'q, 'w, Q, F> {
        let archetype = query.archetypes.get(&query.state.archetype);
        let iter = archetype.map(|a| {
            unsafe {
                Q::iter(a)
            }
        });

        QueryIter {
            iter,
            _marker: PhantomData
        }
    }
}

impl<'q, 'w, Q: QueryBundle, F: FilterGroup> Iterator for QueryIter<'q, 'w, Q, F> {
    type Item = Q::Fetchable<'w>;

    fn next(&mut self) -> Option<Q::Fetchable<'w>> {
        self.iter.as_mut()?.next()
    }
}

impl<'q, 'w, Q: QueryBundle, F: FilterGroup> FusedIterator for QueryIter<'q, 'w, Q, F> {}