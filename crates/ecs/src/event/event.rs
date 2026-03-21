use std::any::{Any, TypeId};

use crate::resource::Resource;

pub trait Event: 'static {
    const NAME: &'static str;
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct EventId(pub(crate) TypeId);

impl EventId {
    #[inline]
    pub const fn of<T: Event>() -> EventId {
        EventId(TypeId::of::<T>())
    }
}

/// Holds the event buffers for a specific event type.
#[derive(Debug, Clone)]
pub struct Events<T: Event> {
    current: Vec<T>,
    previous: Vec<T>,
}

impl<T: Event> Events<T> {
    pub fn new() -> Events<T> {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
        }
    }
}

impl<T: Event> Resource for Events<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

impl<T: Event> Default for Events<T> {
    fn default() -> Events<T> {
        Self::new()
    }
}
