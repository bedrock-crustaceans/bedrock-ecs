use generic_array::GenericArray;
#[cfg(feature = "generics")]
use generic_array::typenum::U0;

#[cfg(feature = "generics")]
use crate::world::World;
use crate::{scheduler::AccessDesc, sealed::Sealed, system::Param};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub(crate) u32);

impl Tick {
    #[inline]
    pub fn get(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemTick {
    this_run: Tick,
    last_run: Tick,
}

impl SystemTick {
    #[inline]
    pub fn this_run(&self) -> Tick {
        self.this_run
    }

    #[inline]
    pub fn last_run(&self) -> Tick {
        self.last_run
    }
}

unsafe impl Param for SystemTick {
    #[cfg(feature = "generics")]
    type AccessCount = U0;

    type State = SystemTick;
    type Output<'a> = SystemTick;

    #[cfg(feature = "generics")]
    fn access(_world: &mut World) -> GenericArray<AccessDesc, U0> {
        unsafe { GenericArray::assume_init(GenericArray::<AccessDesc, U0>::uninit()) }
    }

    #[cfg(not(feature = "generics"))]
    fn access(_world: &mut World) -> SmallVec<[AccessDesc; param::INLINE_SIZE]> {
        SmallVec::new()
    }

    #[inline]
    fn fetch<'a, S: Sealed>(world: &'a World, state: &'a mut SystemTick) -> SystemTick {
        state.last_run = state.this_run;
        state.this_run = Tick(world.current_tick);

        *state
    }

    fn init(world: &mut World, _meta: &crate::system::SystemMeta) -> SystemTick {
        SystemTick {
            last_run: Tick(0),
            this_run: Tick(world.current_tick),
        }
    }
}
