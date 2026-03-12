use std::{any::TypeId, collections::HashMap, iter::FusedIterator, marker::PhantomData, ptr::NonNull};

use smallvec::{SmallVec, smallvec};

use crate::{archetype::{ArchetypeComponents, ArchetypeId, ArchetypeIter, Archetypes}, bitset::BitSet, component::{Component, ComponentId, ComponentRegistry}, entity::{Entity, EntityIter}, filter::FilterGroup, param::{self, Param}, sealed::Sealed, table::{ColumnIter, ColumnIterMut, Table}, world::World};
use crate::graph::{AccessDesc, AccessType};

// pub trait TableIterator: Sized {
//     fn from_table<'t>(table: &'t Table) -> Self;
// }

pub struct ZippedColumnIter<'t, Q: QueryBundle> {
    table: &'t Table,
    index: usize,
    _marker: PhantomData<&'t Q>
}

impl<'t, Q: QueryBundle> Iterator for ZippedColumnIter<'t, Q> {
    type Item = Q::Output<'t>;

    fn next(&mut self) -> Option<Q::Output<'t>> {

        self.index += 1;
        todo!()
    }
}

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
    /// The item that the query outputs.
    type Output<'a> where Self: 'a;
    /// The iterators over the columns.
    type Iter<'a>: Iterator<Item = Self::Output<'a>> where Self: 'a;

    fn archetype(reg: &mut ComponentRegistry) -> BitSet;

    fn access(reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]>;

    fn cache_layout(lookup: &HashMap<TypeId, usize>) -> SmallVec<[usize; 4]>;
}

unsafe impl QueryBundle for Entity<'_> {
    type Output<'w> = Entity<'w> where Self: 'w;
    type Iter<'a> = EntityIter<'a> where Self: 'a;

    fn archetype(_reg: &mut ComponentRegistry) -> BitSet {
        BitSet::new()
    }

    fn access(_reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        smallvec![AccessDesc {
            ty: AccessType::Entity,
            exclusive: true
        }]
    }

    fn cache_layout(_lookup: &HashMap<TypeId, usize>) -> SmallVec<[usize; 4]> {
        SmallVec::new()
    }
}

unsafe impl<T: Component + Send> QueryBundle for &T {
    type Output<'a> = &'a T where Self: 'a;
    type Iter<'a> = ColumnIter<'a, T> where Self: 'a;

    fn archetype(reg: &mut ComponentRegistry) -> BitSet {
        let id = *reg.get_or_assign::<T>();
        let mut bitset = BitSet::with_capacity(id / 64);
        bitset.set(id);
        bitset
    }

    fn access(reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        let id = reg.get_or_assign::<T>();
        smallvec![AccessDesc {
            ty: AccessType::Component(id),
            exclusive: false
        }]
    }

    fn cache_layout(lookup: &HashMap<TypeId, usize>) -> SmallVec<[usize; 4]> {
        let col = *lookup
            .get(&TypeId::of::<T>())
            .expect(&format!("table column lookup failed for component {}", std::any::type_name::<T>()));

        smallvec![col]
    }
}

unsafe impl<T: Component + Send> QueryBundle for &mut T {
    type Output<'a> = &'a mut T where Self: 'a;
    type Iter<'a> = ColumnIterMut<'a, T> where Self: 'a;

    fn archetype(reg: &mut ComponentRegistry) -> BitSet {
        let id = *reg.get_or_assign::<T>();
        let mut bitset = BitSet::with_capacity(id / 64);
        bitset.set(id);
        bitset
    }

    fn access(reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        let id = reg.get_or_assign::<T>();
        smallvec![AccessDesc {
            ty: AccessType::Component(id),
            exclusive: true
        }]
    }

    fn cache_layout(lookup: &HashMap<TypeId, usize>) -> SmallVec<[usize; 4]> {
        let col = *lookup
            .get(&TypeId::of::<T>())
            .expect(&format!("table column lookup failed for component {}", std::any::type_name::<T>()));

        smallvec![col]
    }
}

macro_rules! impl_bundle {
    ($count:expr, $($gen:ident),*) => {
        paste::paste! {
            pub struct [< ZippedIterator $count >]<'t, $($gen),*> {
                index: usize,
                _marker: PhantomData<&'t ($($gen),*)>
            }

            impl<'t, $($gen: ParamRef + Send),*> Iterator for [< ZippedIterator $count >]<'t, $($gen),*> {
                type Item = <($($gen),*) as QueryBundle>::Output<'t>;

                fn next(&mut self) -> Option<Self::Item> {
                    todo!()
                }
            }

            #[diagnostic::do_not_recommend]
            unsafe impl<$($gen: ParamRef + Send),*> QueryBundle for ($($gen),*) {
                type Output<'t> = ($($gen::Output<'t>),*) where Self: 't;
                type Iter<'t> = [< ZippedIterator $count >]<'t, $($gen),*> where Self: 't;

                fn archetype(reg: &mut ComponentRegistry) -> BitSet {
                    let mut bitset = BitSet::new();

                    $(
                        if !$gen::IS_ENTITY {
                            let id = $gen::component_id(reg);
                            bitset.set(*id);    
                        }
                    )*

                    bitset
                }

                fn access(reg: &mut ComponentRegistry) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
                    smallvec![
                        $(
                            $gen::access(reg)
                        ),*
                    ]
                }

                fn cache_layout(lookup: &HashMap<TypeId, usize>) -> SmallVec<[usize; 4]> {
                    const COUNT: usize = (&[$( stringify!($gen) ),*] as &[&str]).len();

                    let mut cache = SmallVec::with_capacity(COUNT);
                    $(
                        if !$gen::IS_ENTITY {
                            cache.push($gen::lookup(lookup));
                        }
                    )*
                    cache
                }
            }
        }
    }
}

impl_bundle!(2, A, B);
impl_bundle!(3, A, B, C);
impl_bundle!(4, A, B, C, D);
impl_bundle!(5, A, B, C, D, E);
impl_bundle!(6, A, B, C, D, E, F);
impl_bundle!(7, A, B, C, D, E, F, G);
impl_bundle!(8, A, B, C, D, E, F, G, H);
impl_bundle!(9, A, B, C, D, E, F, G, H, I);
impl_bundle!(10, A, B, C, D, E, F, G, H, I, J);

pub unsafe trait ParamRef: Send {
    type Unref: 'static;
    type Output<'w>: 'w;

    const IS_ENTITY: bool;

    fn access(reg: &mut ComponentRegistry) -> AccessDesc;
    fn component_id(reg: &mut ComponentRegistry) -> ComponentId;
    fn lookup(map: &HashMap<TypeId, usize>) -> usize;
}

unsafe impl ParamRef for Entity<'_> {
    type Unref = Entity<'static>;
    type Output<'w> = Entity<'w>;

    const IS_ENTITY: bool = true;

    fn access(_reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Entity,
            exclusive: false
        }
    }

    fn component_id(_reg: &mut ComponentRegistry) -> ComponentId {
        unreachable!("attempt to lookup component ID of entity");
    }

    fn lookup(_map: &HashMap<TypeId, usize>) -> usize {
        unimplemented!("attempt to lookup column index of entity");
    }
}

unsafe impl<T: Component + Send + Sync> ParamRef for &T {
    type Unref = T;
    type Output<'w> = &'w T;

    const IS_ENTITY: bool = false;

    fn access(reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc { 
            ty: AccessType::Component(reg.get_or_assign::<T>()), 
            exclusive: false 
        }
    }

    fn component_id(reg: &mut ComponentRegistry) -> ComponentId {
        reg.get_or_assign::<T>()
    }

    fn lookup(map: &HashMap<TypeId, usize>) -> usize {
        let col = *map
            .get(&TypeId::of::<T>())
            .expect(&format!("table column lookup failed for component {}", std::any::type_name::<T>()));

        col
    }
}

unsafe impl<T: Component + Send + Sync> ParamRef for &mut T {
    type Unref = T;
    type Output<'w> = &'w mut T;

    const IS_ENTITY: bool = false;

    fn access(reg: &mut ComponentRegistry) -> AccessDesc {
        AccessDesc {
            ty: AccessType::Component(reg.get_or_assign::<T>()),
            exclusive: true
        }
    }

    fn component_id(reg: &mut ComponentRegistry) -> ComponentId {
        reg.get_or_assign::<T>()
    }

    fn lookup(map: &HashMap<TypeId, usize>) -> usize {
        let col = *map
            .get(&TypeId::of::<T>())
            .expect(&format!("table column lookup failed for component {}", std::any::type_name::<T>()));

        col
    }
}

pub struct Query<'w, Q: QueryBundle, F: FilterGroup = ()> {
    archetypes: &'w Archetypes,
    plan: &'w mut QueryCache<Q, F>,
    _marker: PhantomData<(Q, F)>
}

impl<'w, Q: QueryBundle, F: FilterGroup> Query<'w, Q, F> {
    pub fn new(world: &'w World, state: &'w mut QueryCache<Q, F>) -> Query<'w, Q, F> {
        // Update the plan cache
        state.update(&world.archetypes);

        Query {
            archetypes: &world.archetypes,
            plan: state,
            _marker: PhantomData
        }
    }

    pub fn iter(&self) -> Q::Iter<'_> {
        self.plan.execute(self.archetypes)
    }
}

unsafe impl<'placeholder, Q: QueryBundle + 'static, F: FilterGroup + 'static> Param for Query<'placeholder, Q, F> {
    type State = QueryCache<Q, F>;
    type Output<'w> = Query<'w, Q, F>;

    fn access(world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        Q::access(&mut world.archetypes.registry)
    }

    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut QueryCache<Q, F>) -> Query<'w, Q, F> {
        Query::new(world, state)
    }

    fn init(world: &mut World) -> QueryCache<Q, F> {
        QueryCache::new(&mut world.archetypes)
    }
}

#[derive(Debug)]
pub struct CachedTable {
    /// The table that contains the components.
    pub table: usize,
    /// The columns from this table that contain the components for this query.
    pub cols: SmallVec<[usize; 4]>
}

pub struct QueryCache<Q: QueryBundle, F: FilterGroup> {
    generation: u64,
    archetype: BitSet,
    cached_tables: SmallVec<[CachedTable; 8]>,
    _marker: PhantomData<(Q, F)>
}

impl<Q: QueryBundle, F: FilterGroup> QueryCache<Q, F> {
    pub fn new(archetypes: &mut Archetypes) -> QueryCache<Q, F> {
        let archetype = Q::archetype(&mut archetypes.registry);
        
        let mut cached_tables = SmallVec::new();
        archetypes.cache_tables::<Q>(&archetype, &mut cached_tables);

        QueryCache {
            generation: archetypes.generation(),
            archetype,
            cached_tables,
            _marker: PhantomData
        }
    }

    /// Updates the cache if required.
    pub fn update(&mut self, archetypes: &Archetypes) {
        if self.generation != archetypes.generation() {
            self.cached_tables.clear();
            archetypes.cache_tables::<Q>(&self.archetype, &mut self.cached_tables);
            self.generation = archetypes.generation();
        }
    }

    pub fn execute<'t>(&'t self, archetypes: &'t Archetypes) -> Q::Iter<'t> {
        println!("plan is {:?}", self.cached_tables);

        todo!()

        // QueryIter {
        //     archetypes,
        //     tables: self.cached_tables.iter(),
        //     columns: todo!(),
        //     _marker: PhantomData
        // }
    }
}



pub struct QueryIter<'q, Q: QueryBundle, F: FilterGroup> {
    archetypes: &'q Archetypes,
    cache: std::slice::Iter<'q, CachedTable>,
    table_iter: Q::Iter<'q>,
    _marker: PhantomData<&'q (Q, F)>
}

#[diagnostic::do_not_recommend]
impl<'q, 'w, Q: QueryBundle, F: FilterGroup> IntoIterator for &'q Query<'w, Q, F> {
    type Item = Q::Output<'q>;
    type IntoIter = Q::Iter<'q>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'q, Q: QueryBundle, F: FilterGroup> Iterator for QueryIter<'q, Q, F> {
    type Item = Q::Output<'q>;

    fn next(&mut self) -> Option<Q::Output<'q>> {
        if let Some(next) = self.table_iter.next() {
            return Some(next)
        }

        // Table has ended, jump to next one
        let table_index = self.cache.next()?;
        let table = self.archetypes.table(table_index.table);
    

        todo!()
    }
}