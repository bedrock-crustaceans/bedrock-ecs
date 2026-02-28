use std::{
    any::TypeId,
    marker::PhantomData,
    sync::Arc,
};

use smallvec::SmallVec;

use crate::{scheduler::{BorrowedTypeDescriptor, SystemParamDescriptor}, sealed, Component, EcsResult, Entity, EntityIter, FilterParams, Is, SystemParam, TypedStorage, World};

pub trait QueryParams {
    /// The type that is returned when this param is fetched.
    type Fetchable<'query>;

    const EXCLUSIVE: bool;

    fn descriptor() -> SmallVec<[BorrowedTypeDescriptor; 3]>;
    fn type_id() -> TypeId {
        panic!("This QueryParams implementation does not require the `type_id` function")
    }

    fn fetch<'w>(world: &'w World, entity: Entity) -> Option<Self::Fetchable<'w>>;
    /// Ensures that the entity has the requested components.
    fn filter(entity: &Entity) -> bool;
}

impl QueryParams for Entity {
    type Fetchable<'query> = Entity;

    const EXCLUSIVE: bool = false;

    fn descriptor() -> SmallVec<[BorrowedTypeDescriptor; 3]> {
        SmallVec::new()
    }

    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }

    fn fetch<'w>(_world: &'w World, entity: Entity) -> Option<Self::Fetchable<'w>> {
        Some(entity)
    }

    /// An entity query param needs no filtering as every entity can obviously produce an `Entity` type.
    fn filter(_entity: &Entity) -> bool {
        true
    }
}

impl<T: Component> QueryParams for &T {
    type Fetchable<'query> = &'query T;

    const EXCLUSIVE: bool = false;

    fn descriptor() -> SmallVec<[BorrowedTypeDescriptor; 3]> {
        let mut deps = SmallVec::new();

        deps.push(BorrowedTypeDescriptor {
            exclusive: Self::EXCLUSIVE,
            type_id: TypeId::of::<T>(),
        });

        deps
    }

    fn type_id() -> TypeId {
        TypeId::of::<T>()
    }

    fn fetch<'w>(world: &'w World, entity: Entity) -> Option<Self::Fetchable<'w>> {
        assert_eq!(
            TypeId::of::<&T>(),
            TypeId::of::<Self::Fetchable<'static>>(),
            "QueryParams::Fetchable is incorrect type"
        );

        // Instead of keeping track of lock guards like before, we should instead access the components directly.
        // The scheduler will take care of aliasing issues as it will not schedule mutable queries at the same time as aliased ones.

        let type_id = TypeId::of::<T>();
        let typeless = world.components.map.get(&type_id)?;
        let typed: &TypedStorage<T> = typeless
            .value()
            .as_any()
            .downcast_ref()
            .expect("Failed to downcast typeless storage. The wrong storage type has been inserted into component storage");

        let storage_index = *typed.map.get(&entity.id())?.value();
        let storage = unsafe { &*typed.storage.get() };
        Some(&storage[storage_index])
    }

    fn filter(entity: &Entity) -> bool {
        entity.has::<T>()
    }
}

impl<T: Component> QueryParams for &mut T {
    type Fetchable<'query> = &'query mut T;

    const EXCLUSIVE: bool = true;

    fn descriptor() -> SmallVec<[BorrowedTypeDescriptor; 3]> {
        let mut deps = SmallVec::new();

        deps.push(BorrowedTypeDescriptor {
            exclusive: Self::EXCLUSIVE,
            type_id: TypeId::of::<T>(),
        });

        deps
    }

    fn type_id() -> TypeId {
        TypeId::of::<T>()
    }

    fn fetch<'w>(world: &'w World, entity: Entity) -> Option<Self::Fetchable<'w>> {
        assert_eq!(
            TypeId::of::<&mut T>(),
            TypeId::of::<Self::Fetchable<'static>>(),
            "QueryParams::Fetchable is incorrect type"
        );

        // Instead of keeping track of lock guards like before, we should instead access the components directly.
        // The scheduler will take care of aliasing issues as it will not schedule mutable queries at the same time as aliased ones.

        let type_id = TypeId::of::<T>();

        let typeless = world.components.map.get(&type_id)?;
        let typed: &TypedStorage<T> = typeless
            .value()
            .as_any()
            .downcast_ref()
            .expect("Failed to downcast typeless storage. The wrong storage type has been inserted into component storage");

        let storage_index = *typed.map.get(&entity.id())?.value();
        let storage = unsafe { &mut *typed.storage.get() };
        Some(&mut storage[storage_index])
    }

    fn filter(entity: &Entity) -> bool {
        entity.has::<T>()
    }
}

impl<Q1: QueryParams, Q2: QueryParams> QueryParams for (Q1, Q2) {
    type Fetchable<'query> = (Q1::Fetchable<'query>, Q2::Fetchable<'query>);

    const EXCLUSIVE: bool = Q1::EXCLUSIVE || Q2::EXCLUSIVE;

    fn descriptor() -> SmallVec<[BorrowedTypeDescriptor; 3]> {
        let mut deps = SmallVec::new();

        deps.push(BorrowedTypeDescriptor {
            exclusive: Q1::EXCLUSIVE,
            type_id: Q1::type_id(),
        });

        deps.push(BorrowedTypeDescriptor {
            exclusive: Q2::EXCLUSIVE,
            type_id: Q2::type_id(),
        });

        deps
    }

    fn fetch<'w>(world: &'w World, entity: Entity) -> Option<Self::Fetchable<'w>> {
        let q1 = Q1::fetch(world, entity.clone())?;
        let q2 = Q2::fetch(world, entity)?;

        Some((q1, q2))
    }

    fn filter(entity: &Entity) -> bool {
        Q1::filter(entity) && Q2::filter(entity)
    }
}

pub struct Query<Q: QueryParams, F: FilterParams = ()> {
    world: Arc<World>,
    _marker: PhantomData<(Q, F)>,
}

unsafe impl<Q: QueryParams, F: FilterParams> Send for Query<Q, F> {}
unsafe impl<Q: QueryParams, F: FilterParams> Sync for Query<Q, F> {}

impl<Q: QueryParams, F: FilterParams> Query<Q, F> {
    pub fn new(world: &Arc<World>) -> EcsResult<Self> {
        Ok(Self {
            world: Arc::clone(world),
            _marker: PhantomData,
        })
    }
}

impl<Q: QueryParams, F: FilterParams> SystemParam for Query<Q, F> {
    type State = ();

    fn descriptor() -> SystemParamDescriptor {
        SystemParamDescriptor::Query(Q::descriptor())
    }

    fn fetch<S: sealed::Sealed>(world: &Arc<World>, _state: &Arc<Self::State>) -> Self {
        Query::new(world).expect("Failed to create query")
    }

    fn state(_world: &Arc<World>) -> Arc<Self::State> {
        Arc::new(())
    }
}

impl<'query, Q: QueryParams, F: FilterParams> IntoIterator for &'query Query<Q, F> {
    type Item = Q::Fetchable<'query>;
    type IntoIter = QueryIter<'query, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        QueryIter::from(self)
    }
}

pub struct QueryIter<'query, Q: QueryParams, F: FilterParams> {
    query: &'query Query<Q, F>,
    entities: EntityIter<'query, Q, F>,
}

impl<'query, Q: QueryParams, F: FilterParams> Iterator for QueryIter<'query, Q, F> {
    type Item = Q::Fetchable<'query>;

    fn next(&mut self) -> Option<Self::Item> {
        // Obtain the next entity that matches the filter.
        let entity = self.entities.next()?;
        Q::fetch(&self.query.world, entity)
    }
}

impl<'query, Q: QueryParams, F: FilterParams> From<&'query Query<Q, F>>
    for QueryIter<'query, Q, F>
{
    fn from(query: &'query Query<Q, F>) -> Self {
        let entities = query.world.entities.iter(&query.world);
        QueryIter { query, entities }
    }
}
