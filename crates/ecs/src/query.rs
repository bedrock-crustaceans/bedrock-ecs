use std::{any::TypeId, marker::PhantomData};

use smallvec::{SmallVec, smallvec};
use static_assertions::assert_type_eq_all;

use crate::{component::{Component, ComponentId, Storage}, entity::{Entity, EntityIter}, filter::FilterGroup, param::{Param, ParamDesc, QueryDesc, QueryDescVec, QueryType}, sealed::Sealed, world::World};

pub trait QueryGroup {
    type Fetchable<'a>;

    const SEND: bool;
    const MUTABLE: bool;

    fn desc() -> QueryDescVec;
    fn fetch<'w>(world: &'w World, entity: Entity<'w>) -> Option<Self::Fetchable<'w>>;
    fn filter(entity: &Entity) -> bool;
}

impl QueryGroup for Entity<'_> {
    type Fetchable<'a> = Entity<'a>;

    const SEND: bool = true;
    const MUTABLE: bool = false;

    fn desc() -> QueryDescVec {
        smallvec![QueryDesc {
            mutable: false,
            ty: QueryType::Entity
        }]
    }

    fn fetch<'w>(_world: &'w World, entity: Entity<'w>) -> Option<Self::Fetchable<'w>> {
        Some(entity)
    }

    fn filter(_entity: &Entity<'_>) -> bool {
        true
    }
}

impl<T: Component + Send> QueryGroup for &T {
    type Fetchable<'a> = &'a T;

    const SEND: bool = true;
    const MUTABLE: bool = false;

    fn desc() -> QueryDescVec {
        smallvec![QueryDesc {
            mutable: false,
            ty: QueryType::Component(TypeId::of::<T>())
        }]
    }

    fn fetch<'w>(world: &'w World, entity: Entity<'w>) -> Option<Self::Fetchable<'w>> {
        assert_eq!(
            TypeId::of::<T>(),
            TypeId::of::<Self::Fetchable<'static>>(),
            "&T != Self::Fetchable, this is a bug."
        );

        todo!();
        // let id = ComponentId::of::<T>();
        // let storage: &Storage<T> = world
        //     .components
        //     .map
        //     .get(&id)?
        //     .as_any()
        //     .downcast_ref()
        //     .expect("Invalid storage type has been inserted into component storage");

        // storage.get(entity.id())
    }

    fn filter(entity: &Entity) -> bool {
        entity.has::<T>()
    }
}

impl<T: Component + Send> QueryGroup for &mut T {
    type Fetchable<'a> = &'a mut T;

    const SEND: bool = true;
    const MUTABLE: bool = true;

    fn desc() -> QueryDescVec {
        smallvec![QueryDesc {
            mutable: true,
            ty: QueryType::Component(TypeId::of::<T>())
        }]
    }

    fn fetch<'w>(world: &'w World, entity: Entity<'w>) -> Option<Self::Fetchable<'w>> {
        assert_eq!(
            TypeId::of::<T>(),
            TypeId::of::<Self::Fetchable<'static>>(),
            "&mut T != Self::Fetchable, this is a bug."
        );

        todo!();
        // let type_id = TypeId::of::<T>();
        // let storage: &mut Storage<T> = world
        //     .components
        //     .map
        //     .get(&type_id)?
        //     .as_any_mut()
        //     .downcast_mut()
        //     .expect("Invalid storage type has been inserted into component storage");

        // storage.get_mut(entity.id())
    }

    fn filter(entity: &Entity) -> bool {
        entity.has::<T>()
    }
}

pub struct Query<'w, Q: QueryGroup, F: FilterGroup = ()> {
    world: &'w World,
    _marker: PhantomData<(Q, F)>
}

impl<'w, Q: QueryGroup, F: FilterGroup> Query<'w, Q, F> {
    pub fn new(world: &'w World) -> Query<'w, Q, F> {
        Query {
            world,
            _marker: PhantomData
        }
    }
}

impl<'placeholder, Q: QueryGroup, F: FilterGroup> Param for Query<'placeholder, Q, F> {
    type State = ();
    type Item<'w> = Query<'w, Q, F>;

    const SEND: bool = Q::SEND;

    fn desc() -> ParamDesc {
        ParamDesc::Query(Q::desc())
    }

    fn fetch<'w, S: Sealed>(world: &'w World, _: &mut ()) -> Query<'w, Q, F> {
        Query {
            world,
            _marker: PhantomData
        }
    }

    fn init() {}
    fn destroy(_: &mut ()) {}
}

impl<'q, 'w, Q: QueryGroup, F: FilterGroup> IntoIterator for &'q Query<'w, Q, F> {
    type Item = Q::Fetchable<'w>;
    type IntoIter = QueryIter<'q, 'w, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        QueryIter::from(self)
    }
}

pub struct QueryIter<'q, 'w, Q: QueryGroup, F: FilterGroup> {
    query: &'q Query<'w, Q, F>,
    entities: EntityIter<'q, Q, F>
}

impl<'q, 'w, Q: QueryGroup, F: FilterGroup> Iterator for QueryIter<'q, 'w, Q, F> {
    type Item = Q::Fetchable<'w>;

    fn next(&mut self) -> Option<Q::Fetchable<'w>> {
        let entity = self.entities.next()?;
        todo!()
        // Q::fetch(&self.world, entity)
    }
}

impl<'q, 'w, Q: QueryGroup, F: FilterGroup> From<&'q Query<'w, Q, F>> for QueryIter<'q, 'w, Q, F> {
    fn from(query: &'q Query<'w, Q, F>) -> QueryIter<'q, 'w, Q, F> {
        let entities = todo!();
        QueryIter { query, entities }
    }
}