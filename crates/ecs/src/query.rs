use std::{any::TypeId, iter::FusedIterator, marker::PhantomData, ptr::NonNull};

use crate::{archetype::{ArchetypeComponents, ArchetypeId, ArchetypeIter, Archetypes}, component::{Component, ComponentId}, entity::{Entity, EntityIter}, filter::FilterGroup, param::Param, sealed::Sealed, table::{ColumnIter, ColumnIterMut, Table}, world::World};
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
    type ElemPtr<'w>;
    type Iter<'w>: Iterator<Item = Self::ElemPtr<'w>>;

    fn archetype() -> ArchetypeComponents;

    fn access() -> Vec<AccessDesc>;

    unsafe fn iter<'t>(table: &'t Archetypes) -> Self::Iter<'t>;

    unsafe fn from_ptr<'t>(ptr: Self::ElemPtr<'t>) -> Self::Output<'t>;
}

unsafe impl QueryBundle for Entity<'_> {
    type Output<'w> = Entity<'w>;
    type ElemPtr<'w> = Entity<'w>;
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

    unsafe fn iter(_table: &Archetypes) -> EntityIter {
        todo!()
    }

    unsafe fn from_ptr<'t>(ptr: Self::ElemPtr<'t>) -> Self::Output<'t> {
        ptr
    }
}

unsafe impl<T: Component + Send> QueryBundle for &T {
    type Output<'a> = &'a T;
    type ElemPtr<'w> = NonNull<T>;
    type Iter<'w> = ArchetypeIter<'w, T>;

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<T>()]))
    }

    fn access() -> Vec<AccessDesc> {
        vec![AccessDesc {
            ty: AccessType::Component(ComponentId::of::<T>()),
            exclusive: false
        }]
    }

    unsafe fn iter(table: &Archetypes) -> ArchetypeIter<'_, T> {
        todo!()
    }

    unsafe fn from_ptr<'t>(ptr: NonNull<T>) -> &'t T {
        unsafe { &*(ptr.as_ptr().cast_const()) }
    }
}

unsafe impl<T: Component + Send> QueryBundle for &mut T {
    type Output<'a> = &'a mut T;
    type ElemPtr<'w> = NonNull<T>;
    type Iter<'w> = ArchetypeIter<'w, T>;

    fn archetype() -> ArchetypeComponents {
        ArchetypeComponents(Box::new([ComponentId::of::<T>()]))
    }

    fn access() -> Vec<AccessDesc> {
        vec![AccessDesc {
            ty: AccessType::Component(ComponentId::of::<T>()),
            exclusive: true
        }]
    }

    unsafe fn iter(table: &Archetypes) -> ArchetypeIter<'_, T> {
        todo!()
    }

    unsafe fn from_ptr<'w>(ptr: NonNull<T>) -> &'w mut T {
        unsafe { &mut *ptr.as_ptr() }
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
impl_iter!(A, B, C, D, E, F);
impl_iter!(A, B, C, D, E, F, G);
impl_iter!(A, B, C, D, E, F, G, H);
impl_iter!(A, B, C, D, E, F, G, H, I);
impl_iter!(A, B, C, D, E, F, G, H, I, J);

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
        #[diagnostic::do_not_recommend]
        unsafe impl<$($gen: ParamRef + Send),*> QueryBundle for ($($gen),*) {
            type Output<'t> = ($($gen::Output<'t>),*);
            // type ElemPtr<'t> = ($(NonNull<$gen>),*);
            type ElemPtr<'t> = &'static ();
            type Iter<'t> = std::slice::Iter<'static, ()>;

            fn archetype() -> ArchetypeComponents {
                let comps: Box<[ComponentId]> = [$($gen::component_id()),*]
                    .into_iter().flatten().collect();

                ArchetypeComponents(comps)
            }

            fn access() -> Vec<AccessDesc> {
                vec![$($gen::access()),*]
            }

            unsafe fn iter<'t>(table: &'t Archetypes) -> Self::Iter<'t> {
                todo!()
            }

            // unsafe fn from_ptr<'t>(ptr: ($(NonNull<$gen>),*)) -> Self::Output<'t> {
            //     todo!()
            // }
            unsafe fn from_ptr<'t>(ptr: &()) -> Self::Output<'t> {
                todo!()
            }
        }
    }
}

impl_bundle!(A, B);
impl_bundle!(A, B, C);
impl_bundle!(A, B, C, D);
impl_bundle!(A, B, C, D, E);
impl_bundle!(A, B, C, D, E, F);
impl_bundle!(A, B, C, D, E, F, G);
impl_bundle!(A, B, C, D, E, F, G, H);
impl_bundle!(A, B, C, D, E, F, G, H, I);
impl_bundle!(A, B, C, D, E, F, G, H, I, J);

pub struct Query<'w, Q: QueryBundle, F: FilterGroup = ()> {
    archetypes: &'w Archetypes,
    plan: &'w QueryPlan<Q, F>,
    _marker: PhantomData<(Q, F)>
}

impl<'w, Q: QueryBundle, F: FilterGroup> Query<'w, Q, F> {
    pub fn new(world: &'w World, state: &'w QueryPlan<Q, F>) -> Query<'w, Q, F> {
        Query {
            archetypes: &world.archetypes,
            plan: state,
            _marker: PhantomData
        }
    }

    pub fn iter(&self) -> QueryIter<'_, Q, F> {
        // self.plan.update(self.archetypes);

        todo!()
        // Q::iter(self)
    }
}

unsafe impl<'placeholder, Q: QueryBundle + 'static, F: FilterGroup + 'static> Param for Query<'placeholder, Q, F> {
    type State = QueryPlan<Q, F>;
    type Output<'w> = Query<'w, Q, F>;

    fn access() -> Vec<AccessDesc> {
        Q::access()
    }

    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut QueryPlan<Q, F>) -> Query<'w, Q, F> {
        Query::new(world, state)
    }

    fn init() -> QueryPlan<Q, F> {
        QueryPlan::new()
    }
    
    fn destroy(_: &mut QueryPlan<Q, F>) {}
}

pub struct QueryPlan<Q: QueryBundle, F: FilterGroup> {
    generation: u64,
    // TODO: This could maybe be a smallvec.
    cached_tables: Vec<ArchetypeId>,
    _marker: PhantomData<(Q, F)>
}

impl<Q: QueryBundle, F: FilterGroup> QueryPlan<Q, F> {
    pub fn new() -> QueryPlan<Q, F> {
        QueryPlan {
            generation: u64::MAX,
            cached_tables: Vec::new(),
            _marker: PhantomData
        }
    }

    /// Updates the cache if required.
    pub fn update(&mut self, archetypes: &Archetypes) {
        if self.generation != archetypes.generation() {
            self.cached_tables.clear();
            archetypes.cache_tables(&mut self.cached_tables);
            self.generation = archetypes.generation();
        }
    }

    pub fn execute<'t>(&'t self, archetypes: &'t Archetypes) -> QueryIter<'t, Q, F> {
        QueryIter {
            archetypes,
            tables: self.cached_tables.iter(),
            column_index: 0,
            table_index: ArchetypeId(0),
            _marker: PhantomData
        }
    }
}

pub struct QueryIter<'q, Q: QueryBundle, F: FilterGroup> {
    archetypes: &'q Archetypes,
    tables: std::slice::Iter<'q, ArchetypeId>,

    table_index: ArchetypeId,
    column_index: usize,
    _marker: PhantomData<&'q (Q, F)>
}

#[diagnostic::do_not_recommend]
impl<'q, 'w, Q: QueryBundle, F: FilterGroup> IntoIterator for &'q Query<'w, Q, F> {
    type Item = Q::Output<'q>;
    type IntoIter = QueryIter<'q, Q, F>;

    fn into_iter(self) -> Self::IntoIter {
        todo!("update plan");
        // self.plan.update();
        self.plan.execute(self.archetypes)
    }
}

impl<'q, Q: QueryBundle, F: FilterGroup> Iterator for QueryIter<'q, Q, F> {
    type Item = Q::Output<'q>;

    fn next(&mut self) -> Option<Q::Output<'q>> {
        todo!()
        // self.iter.as_mut()?.next()
    }
}