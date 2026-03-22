use std::{any::TypeId, marker::PhantomData};

use rustc_hash::FxHashMap;

use crate::{
    component::{Component, ComponentId, ComponentRegistry},
    query::{EmptyableIterator, FilterBundle, Impossible, QueryData, QueryType},
    scheduler::AccessDesc,
    world::World,
};

pub struct Has<T: Component> {
    _marker: PhantomData<T>,
}

unsafe impl<T: Component> QueryData for Has<T> {
    type Unref = Has<T>;

    type Output<'w> = bool;
    type Iter<'t, F: FilterBundle> = Impossible<bool>;

    const TY: QueryType = QueryType::Has;

    fn access(_reg: &mut ComponentRegistry) -> AccessDesc {
        unimplemented!("`Has` does not access any resources");
    }

    fn component_id(_reg: &mut ComponentRegistry) -> ComponentId {
        unimplemented!()
    }

    fn cache_column(_map: &FxHashMap<TypeId, usize>) -> usize {
        unimplemented!()
    }

    fn iter<F: FilterBundle>(
        world: &World,
        table: usize,
        _col: usize,
        _last_tick: u32,
        _current_tick: u32,
    ) -> Impossible<bool> {
        unimplemented!()
    }
}
