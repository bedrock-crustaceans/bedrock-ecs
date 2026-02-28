use std::marker::PhantomData;

use crate::{param::{Param, ParamDesc}, sealed::Sealed};

pub struct Local<T: 'static> {
    _marker: PhantomData<T>
}

impl<T> Param for Local<T> {
    type State = Local<T>;

    fn desc() -> ParamDesc {
        ParamDesc::Local
    }

    fn init(state: &Local<T>) {}
    fn destroy(state: &Local<T>) {}

    fn fetch<S: Sealed>(state: &Local<T>) -> Self {
        todo!()
    }

    fn state(&self) -> &Local<T> {
        todo!()
    }
}