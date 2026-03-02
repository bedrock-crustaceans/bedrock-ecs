use std::{alloc::{Layout}};

pub fn repeat_layout(layout: Layout, n: usize) -> Layout {
    let size = layout.size();
    let align = layout.align();

    let padded = (size + align - 1) & !(align - 1);
    let total = padded.checked_mul(n).expect("Array capacity has overflowed");

    Layout::from_size_align(total, align).expect("Invalid array layout")
}

#[cfg(debug_assertions)]
pub mod debug {
    use std::sync::atomic::{AtomicU8, Ordering};

    const UNLOCKED: u8 = 0;
    const READ: u8 = 1;
    const WRITE: u8 = 2;

    #[derive(Debug)]
    pub struct RwFlag {
        state: AtomicU8
    }

    impl RwFlag {
        pub fn new() -> RwFlag {
            RwFlag {
                state: AtomicU8::new(UNLOCKED)
            }
        }

        pub fn read(&self) {
            let prev = self.state.fetch_max(1, Ordering::SeqCst);
            assert_eq!(prev, UNLOCKED, "RwFlag was write, cannot read");
            println!("Lock read");
        }

        pub fn write(&self) {
            let prev = self.state.fetch_add(2, Ordering::SeqCst);
            assert_eq!(prev, UNLOCKED, "RwFlag was not unlocked, cannot write");
            println!("Lock write");
        }

        pub fn unlock(&self) {
            let prev = self.state.fetch_min(0, Ordering::SeqCst);
            assert_ne!(prev, UNLOCKED, "Cannot unlock RwFlag twice");
            println!("Unlock");
        }
    }
}