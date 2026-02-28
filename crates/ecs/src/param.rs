use crate::{sealed::Sealed, world::World};

pub enum ParamDesc {
    Unit,
    Local
}

pub trait Param {
    type State;
    type Item<'w>;

    fn desc() -> ParamDesc;

    #[doc(hidden)]
    fn fetch<'w, S: Sealed>(world: &'w World, state: &Self::State) -> Self::Item<'w>;

    fn state(&self) -> &Self::State;

    fn init(state: &Self::State);

    fn destroy(state: &Self::State);
}

pub trait ParamGroup {
    type State;

    fn init() -> Self::State;
}

impl Param for () {
    type State = ();
    type Item<'w> = ();

    fn desc() -> ParamDesc {
        ParamDesc::Unit
    }

    fn state(&self) -> &() { &() }

    fn fetch<'w, S: Sealed>(_world: &'w World, _state: &Self::State) -> Self::Item<'w> {}

    fn init(_state: &Self::State) {}
    fn destroy(_state: &Self::State) {}
}

impl<P: Param> ParamGroup for P {
    type State = P::State;

    fn init() -> Self::State {
        todo!()        
    }
}

impl<P1: Param, P2: Param> ParamGroup for (P1, P2) {
    type State = (P1::State, P2::State);

    fn init() -> Self::State {
        todo!()
    }
}

impl<P1: Param, P2: Param, P3: Param> ParamGroup for (P1, P2, P3) {
    type State = (P1::State, P2::State, P3::State);

    fn init() -> Self::State {
        todo!()
    }
}
