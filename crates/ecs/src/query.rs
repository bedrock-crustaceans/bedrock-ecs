use std::{any::TypeId, iter::FusedIterator, marker::PhantomData, ptr::NonNull};

use smallvec::{SmallVec, smallvec};

use crate::{archetype::{ArchetypeComponents, Archetypes}, component::{Component, ComponentId}, entity::{Entity, EntityIter}, filter::FilterGroup, param::{Param}, sealed::Sealed, table::{ColumnIter, ColumnIterMut, Table}, world::World};
use crate::graph::{AccessDesc, AccessType};

/// # Safety:
///
/// The `access` method must correctly return the types this query uses.
/// Incorrect implementation will lead to reference aliasing and inevitable UB.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid query type", 
    label = "invalid query",
    // note = "only `Entity`, `&T` and `&mut T` where `T: Component` or tuples thereof can be used in queries",
    note = "components in a query must be wrapped in a reference, e.g. `&{Self}` or `&mut {Self}`",
    note = "if `{Self}` is a component, do not forget to implement the `Component` trait"
)]
pub unsafe trait QueryGroup {
    type Fetchable<'w>;
    type Iter<'w>: Iterator<Item = Self::Fetchable<'w>>;

    const SEND: bool;
    const MUTABLE: bool;

    fn archetype() -> ArchetypeComponents;
    unsafe fn iter<'w>(table: &'w Table) -> Self::Iter<'w>;

    unsafe fn from_ptr<'w>(ptr: NonNull<u8>) -> Self::Fetchable<'w>;
    fn access() -> Vec<AccessDesc>;
}

unsafe impl QueryGroup for Entity<'_> {
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

    fn access() -> Vec<AccessDesc> {
        vec![AccessDesc {
            ty: AccessType::Entity,
            exclusive: true
        }]
    }
}

unsafe impl<T: Component + Send> QueryGroup for &T {
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

    fn access() -> Vec<AccessDesc> {
        vec![AccessDesc {
            ty: AccessType::Component(ComponentId::of::<T>()),
            exclusive: false
        }]
    }
}

unsafe impl<T: Component + Send> QueryGroup for &mut T {
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

    fn access() -> Vec<AccessDesc> {
        vec![AccessDesc {
            ty: AccessType::Component(ComponentId::of::<T>()),
            exclusive: true
        }]
    }
}

pub struct JoinedIter<'w, T> {
    _marker: PhantomData<&'w T>
}

impl<'w, T1, T2> Iterator for JoinedIter<'w, (T1, T2)> {
    type Item = (T1, T2);

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

pub unsafe trait ParamRef: Send {
    type Unref;
    type Fetchable<'w>: 'w;
    type Iter<'w>: Iterator<Item = Self::Fetchable<'w>>;

    const EXCLUSIVE: bool;

    fn access() -> AccessDesc;

    fn component_id() -> Option<ComponentId>;
}

unsafe impl ParamRef for Entity<'_> {
    type Unref = Entity<'static>;
    type Fetchable<'w> = Entity<'w>;
    type Iter<'w> = EntityIter<'w>;

    const EXCLUSIVE: bool = false;

    fn access() -> AccessDesc {
        AccessDesc {
            ty: AccessType::Entity,
            exclusive: false
        }
    }

    fn component_id() -> Option<ComponentId> {
        None
    }
}

unsafe impl<T: Component + Send + Sync> ParamRef for &T {
    type Unref = T;
    type Fetchable<'w> = &'w T;
    type Iter<'w> = ColumnIter<'w, T>;

    const EXCLUSIVE: bool = false;

    fn access() -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(ComponentId::of::<T>()),
            exclusive: false
        }
    }

    fn component_id() -> Option<ComponentId> {
        Some(ComponentId::of::<T>())
    }
}

unsafe impl<T: Component + Send + Sync> ParamRef for &mut T {
    type Unref = T;
    type Fetchable<'w> = &'w mut T;
    type Iter<'w> = ColumnIterMut<'w, T>;

    const EXCLUSIVE: bool = true;

    fn access() -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(ComponentId::of::<T>()),
            exclusive: true
        }
    }

    fn component_id() -> Option<ComponentId> {
        Some(ComponentId::of::<T>())
    }
}

unsafe impl<T1: ParamRef + Send, T2: ParamRef + Send> QueryGroup for (T1, T2) {
    type Fetchable<'w> = (T1::Fetchable<'w>, T2::Fetchable<'w>);
    type Iter<'w> = JoinedIter<'w, (T1::Fetchable<'w>, T2::Fetchable<'w>)>;

    const SEND: bool = true;
    const MUTABLE: bool = false;

    fn archetype() -> ArchetypeComponents {
        let c1 = T1::component_id();
        let c2 = T2::component_id();

        // Only store the ids that are not `None`.
        let comps: Box<[ComponentId]> = [c1, c2]
            .into_iter()
            .flatten()
            .collect();

        ArchetypeComponents(comps)
    }

    unsafe fn iter<'w>(table: &'w Table) -> Self::Iter<'w> {
        todo!()
    }

    unsafe fn from_ptr<'w>(ptr: NonNull<u8>) -> Self::Fetchable<'w> {
        todo!()
    }

    fn access() -> Vec<AccessDesc> {
        vec![T1::access(), T2::access()]
    }
}

pub struct Query<'w, Q: QueryGroup, F: FilterGroup = ()> {
    archetypes: &'w Archetypes,
    state: &'w QueryState,
    _marker: PhantomData<(Q, F)>
}

impl<'w, Q: QueryGroup, F: FilterGroup> Query<'w, Q, F> {
    pub fn new(world: &'w World, state: &'w QueryState) -> Query<'w, Q, F> {
        println!("Query mutable? {}", Q::MUTABLE);

        Query {
            archetypes: &world.archetypes,
            state,
            _marker: PhantomData
        }
    }
}

unsafe impl<'placeholder, Q: QueryGroup, F: FilterGroup> Param for Query<'placeholder, Q, F> {
    type State = QueryState;
    type Item<'w> = Query<'w, Q, F>;

    const SEND: bool = Q::SEND;

    fn access() -> Vec<AccessDesc> {
        Q::access()
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

pub struct QueryIter<'q, 'w, Q: QueryGroup, F: FilterGroup> {
    iter: Option<Q::Iter<'w>>,
    _marker: PhantomData<&'q (Q, F)>
}

#[diagnostic::do_not_recommend]
impl<'q, 'w, Q: QueryGroup, F: FilterGroup> IntoIterator for &'q Query<'w, Q, F> {
    type Item = Q::Fetchable<'w>;
    type IntoIter = QueryIter<'q, 'w, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        QueryIter::from(self)
    }
}

impl<'q, 'w, Q: QueryGroup, F: FilterGroup> From<&'q Query<'w, Q, F>> for QueryIter<'q, 'w, Q, F> {
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

impl<'q, 'w, Q: QueryGroup, F: FilterGroup> Iterator for QueryIter<'q, 'w, Q, F> {
    type Item = Q::Fetchable<'w>;

    fn next(&mut self) -> Option<Q::Fetchable<'w>> {
        self.iter.as_mut()?.next()
    }
}

impl<'q, 'w, Q: QueryGroup, F: FilterGroup> FusedIterator for QueryIter<'q, 'w, Q, F> {}