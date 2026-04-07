use generic_array::GenericArray;
#[cfg(feature = "generics")]
use generic_array::typenum::U0;

#[cfg(feature = "generics")]
use crate::world::World;
use crate::{
    scheduler::AccessDesc,
    sealed::Sealed,
    system::{SysArg, SysMeta},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub(crate) u32);

impl Tick {
    #[inline]
    pub fn get(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TickInfo {
    this_run: Tick,
    last_run: Tick,
}

impl TickInfo {
    /// The current world tick
    #[inline]
    pub fn this_run(&self) -> Tick {
        self.this_run
    }

    /// The world tick from the last time this system ran. If the system has never run before, this
    /// will be 0.
    #[inline]
    pub fn last_run(&self) -> Tick {
        self.last_run
    }
}

unsafe impl SysArg for TickInfo {
    #[cfg(feature = "generics")]
    type AccessCount = U0;

    type State = TickInfo;
    type Output<'a> = TickInfo;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U0> {
        unsafe { GenericArray::assume_init(GenericArray::<AccessDesc, U0>::uninit()) }
    }

    #[cfg(not(feature = "generics"))]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; SysArg::INLINE_SIZE]> {
        SmallVec::new()
    }

    fn before_update<'w>(world: &'w World, state: &'w mut TickInfo) -> TickInfo {
        state.this_run = Tick(world.current_tick);

        *state
    }

    fn after_update(world: &World, state: &mut Self::State) {
        state.last_run = state.this_run;
    }

    fn init(world: &mut World, _meta: &SysMeta) -> TickInfo {
        TickInfo {
            last_run: Tick(0),
            this_run: Tick(world.current_tick),
        }
    }
}
