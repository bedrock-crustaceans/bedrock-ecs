use std::marker::PhantomData;

use crate::{component::Component, entity::Entity};

pub trait Filter {
    fn filter(entity: &Entity) -> bool;
}

pub trait FilterGroup {
    fn filter(entity: &Entity) -> bool;
}

impl Filter for () {
    fn filter(_entity: &Entity) -> bool {
        true
    }
}

impl<F: Filter> FilterGroup for F {
    fn filter(entity: &Entity) -> bool {
        F::filter(entity)
    }
}

impl<F1: Filter, F2: Filter> FilterGroup for (F1, F2) {
    fn filter(entity: &Entity) -> bool {
        F1::filter(entity) && F2::filter(entity)
    }
}

pub struct With<T> {
    _marker: PhantomData<T>
}

pub struct Without<T> {
    _marker: PhantomData<T>
}

pub struct Added<T> {
    _marker: PhantomData<T>
}

pub struct Removed<T> {
    _marker: PhantomData<T>
}

pub struct Changed<T> {
    _marker: PhantomData<T>
}

impl<T: Component> Filter for With<T> {
    fn filter(entity: &Entity) -> bool {
        entity.has::<T>()
    }
}

impl<T: Component> Filter for Without<T> {
    fn filter(entity: &Entity) -> bool {
        !entity.has::<T>()
    }
}