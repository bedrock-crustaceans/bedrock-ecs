use std::any::TypeId;

use smallvec::SmallVec;

use crate::{sealed::Sealed, world::World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryType {
    Entity,
    Component(TypeId)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryDesc {
    pub(crate) ty: QueryType,
    pub(crate) mutable: bool
}

pub type QueryDescVec = SmallVec<[QueryDesc; 3]>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamDesc {
    Unit,
    Local,
    Query(QueryDescVec)
}

#[diagnostic::on_unimplemented(
    message = "{Self} is not a valid system parameter"
)]
pub trait Param {
    type State;
    type Item<'w>;

    const SEND: bool;

    fn desc() -> ParamDesc;

    #[doc(hidden)]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &'w mut Self::State) -> Self::Item<'w>;

    fn init() -> Self::State;

    fn destroy(state: &mut Self::State);
}

pub trait ParamBundle {
    type State;

    const SEND: bool;

    fn init() -> Self::State;
}

impl Param for () {
    type State = ();
    type Item<'w> = ();

    const SEND: bool = true;

    fn desc() -> ParamDesc {
        ParamDesc::Unit
    }

    fn fetch<'w, S: Sealed>(_world: &'w World, _state: &'w mut Self::State) -> Self::Item<'w> {}

    fn init() {}
    fn destroy(_state: &mut Self::State) {}
}

impl<P: Param> ParamBundle for P {
    type State = P::State;

    const SEND: bool = P::SEND;

    fn init() -> Self::State {
        P::init()     
    }
}

impl<P1: Param, P2: Param> ParamBundle for (P1, P2) {
    type State = (P1::State, P2::State);

    const SEND: bool = P1::SEND && P2::SEND;

    fn init() -> Self::State {
        (P1::init(), P2::init())
    }
}

impl<P1: Param, P2: Param, P3: Param> ParamBundle for (P1, P2, P3) {
    type State = (P1::State, P2::State, P3::State);

    const SEND: bool = P1::SEND && P2::SEND && P3::SEND;

    fn init() -> Self::State {
        (P1::init(), P2::init(), P3::init())
    }
}
