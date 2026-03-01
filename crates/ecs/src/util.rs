use std::{alloc::{Layout}};

pub fn repeat_layout(layout: Layout, n: usize) -> Layout {
    let size = layout.size();
    let align = layout.align();

    let padded = (size + align - 1) & !(align - 1);
    let total = padded.checked_mul(n).expect("Array capacity has overflowed");

    Layout::from_size_align(total, align).expect("Invalid array layout")
}