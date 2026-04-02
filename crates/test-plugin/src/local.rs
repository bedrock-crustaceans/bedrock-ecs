use std::ops::{Deref, DerefMut};

pub struct Local<T>(pub(crate) T);

impl<T> Deref for Local<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Local<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
