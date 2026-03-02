use std::any::TypeId;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct ComponentId(pub(crate) TypeId);

impl ComponentId {
    pub fn of<T: Component>() -> ComponentId {
        ComponentId(TypeId::of::<T>())
    }
}

pub trait Component: 'static {}