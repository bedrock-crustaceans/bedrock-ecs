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
pub unsafe trait QueryBundle {
    type Output<'w>;
    type Iter<'w>: Iterator<Item = Self::Output<'w>>;

    fn archetype() -> ArchetypeComponents;

    fn access() -> Vec<AccessDesc>;

    unsafe fn iter<'t>(table: &'t Table) -> Self::Iter<'t>;

    unsafe fn from_ptr<'t>(ptr: NonNull<u8>) -> Self::Output<'t>;

}

unsafe impl QueryBundle for Entity<'_> {
    type Output<'a> = Entity<'a>;
    type Iter<'w> = EntityIter<'w>;

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([]))
    }

    fn access() -> Vec<AccessDesc> {
        vec![AccessDesc {
            ty: AccessType::Entity,
            exclusive: true
        }]
    }

    unsafe fn iter(_table: &Table) -> EntityIter {
        todo!()
    }

    unsafe fn from_ptr<'w>(_ptr: NonNull<u8>) -> Entity<'w> {
        panic!("Cannot instantiate Entity from pointer");
    }
}

unsafe impl<T: Component + Send> QueryBundle for &T {
    type Output<'a> = &'a T;
    type Iter<'w> = ColumnIter<'w, T>;

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<T>()]))
    }

    fn access() -> Vec<AccessDesc> {
        vec![AccessDesc {
            ty: AccessType::Component(ComponentId::of::<T>()),
            exclusive: false
        }]
    }

    unsafe fn iter(table: &Table) -> ColumnIter<T> {
        let id = ComponentId::of::<T>();
        let col = table.col(&id);
        ColumnIter::new(col)
    }

    unsafe fn from_ptr<'w>(ptr: NonNull<u8>) -> &'w T {
        unsafe { &*(ptr.as_ptr() as *const T) }
    }
}

unsafe impl<T: Component + Send> QueryBundle for &mut T {
    type Output<'a> = &'a mut T;
    type Iter<'w> = ColumnIterMut<'w, T>;

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<T>()]))
    }

    fn access() -> Vec<AccessDesc> {
        vec![AccessDesc {
            ty: AccessType::Component(ComponentId::of::<T>()),
            exclusive: true
        }]
    }

    unsafe fn iter(table: &Table) -> ColumnIterMut<T> {
        todo!()
    }

    unsafe fn from_ptr<'w>(ptr: NonNull<u8>) -> &'w mut T {
        unsafe { &mut *(ptr.as_ptr() as *mut T) }
    }
}

pub struct JoinedIter<'w, T> {
    _marker: PhantomData<&'w T>
}

macro_rules! impl_iter {
    ($($gen:ident),*) => {
        impl<'t, $($gen),*> Iterator for JoinedIter<'t, ($($gen),*)> {
            type Item = ($($gen),*);

            fn next(&mut self) -> Option<Self::Item> {
                todo!()
            }
        }
    }
}

impl_iter!(A, B);
impl_iter!(A, B, C);
impl_iter!(A, B, C, D);
impl_iter!(A, B, C, D, E);

pub unsafe trait ParamRef: Send {
    type Unref;
    type Output<'w>: 'w;
    type Iter<'w>: Iterator<Item = Self::Output<'w>>;

    const EXCLUSIVE: bool;

    fn access() -> AccessDesc;

    fn component_id() -> Option<ComponentId>;
}

unsafe impl ParamRef for Entity<'_> {
    type Unref = Entity<'static>;
    type Output<'w> = Entity<'w>;
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
    type Output<'w> = &'w T;
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
    type Output<'w> = &'w mut T;
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

macro_rules! impl_bundle {
    ($($gen:ident),*) => {
        unsafe impl<$($gen: ParamRef + Send),*> QueryBundle for ($($gen),*) {
            type Output<'t> = ($($gen::Output<'t>),*);
            type Iter<'t> = JoinedIter<'t, ($($gen::Output<'t>),*)>;

            fn archetype() -> ArchetypeComponents {
                let comps: Box<[ComponentId]> = [$($gen::component_id()),*]
                    .into_iter().flatten().collect();

                ArchetypeComponents(comps)
            }

            fn access() -> Vec<AccessDesc> {
                vec![$($gen::access()),*]
            }

            unsafe fn iter<'t>(table: &'t Table) -> Self::Iter<'t> {
                todo!()
            }

            unsafe fn from_ptr<'t>(ptr: NonNull<u8>) -> Self::Output<'t> {
                todo!()
            }
        }
    }
}

impl_bundle!(A, B);
impl_bundle!(A, B, C);
impl_bundle!(A, B, C, D);
impl_bundle!(A, B, C, D, E);

// unsafe impl<T1: ParamRef + Send, T2: ParamRef + Send> QueryBundle for (T1, T2) {
//     type Output<'w> = (T1::Output<'w>, T2::Output<'w>);
//     type Iter<'w> = JoinedIter<'w, (T1::Output<'w>, T2::Output<'w>)>;
//
//     fn archetype() -> ArchetypeComponents {
//         let c1 = T1::component_id();
//         let c2 = T2::component_id();
//
//         // Only store the ids that are not `None`.
//         let comps: Box<[ComponentId]> = [c1, c2]
//             .into_iter()
//             .flatten()
//             .collect();
//
//         ArchetypeComponents(comps)
//     }
//
//     unsafe fn iter<'w>(table: &'w Table) -> Self::Iter<'w> {
//         todo!()
//     }
//
//     unsafe fn from_ptr<'w>(ptr: NonNull<u8>) -> Self::Output<'w> {
//         todo!()
//     }
//
//     fn access() -> Vec<AccessDesc> {
//         vec![T1::access(), T2::access()]
//     }
// }

pub struct Query<'w, Q: QueryBundle, F: FilterGroup = ()> {
    archetypes: &'w Archetypes,
    state: &'w QueryState,
    _marker: PhantomData<(Q, F)>
}

impl<'w, Q: QueryBundle, F: FilterGroup> Query<'w, Q, F> {
    pub fn new(world: &'w World, state: &'w QueryState) -> Query<'w, Q, F> {
        Query {
            archetypes: &world.archetypes,
            state,
            _marker: PhantomData
        }
    }
}

unsafe impl<'placeholder, Q: QueryBundle, F: FilterGroup> Param for Query<'placeholder, Q, F> {
    type State = QueryState;
    type Output<'w> = Query<'w, Q, F>;

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

pub struct QueryIter<'q, 'w, Q: QueryBundle, F: FilterGroup> {
    iter: Option<Q::Iter<'w>>,
    _marker: PhantomData<&'q (Q, F)>
}

#[diagnostic::do_not_recommend]
impl<'q, 'w, Q: QueryBundle, F: FilterGroup> IntoIterator for &'q Query<'w, Q, F> {
    type Item = Q::Output<'w>;
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
    type Item = Q::Output<'w>;

    fn next(&mut self) -> Option<Q::Output<'w>> {
        self.iter.as_mut()?.next()
    }
}

impl<'q, 'w, Q: QueryBundle, F: FilterGroup> FusedIterator for QueryIter<'q, 'w, Q, F> {}