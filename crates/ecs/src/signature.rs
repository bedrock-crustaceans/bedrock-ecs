use smallvec::SmallVec;

const WORD_COUNT: usize = 4;
const SIMD_LANES: usize = 4;

/// A set of bits, similar to `Vec<bool>` but more efficient with memory.
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct Signature {
    // bits: SmallVec<[u64; INLINE_COUNT]>
    bits: [u64; WORD_COUNT],
}

impl Signature {
    /// Creates a new, empty bitset.
    pub fn new() -> Signature {
        Signature::default()
    }

    /// Whether this bitset is empty.
    ///
    /// Empty can mean that it either has no words or all bits are set to 0.
    pub fn is_empty(&self) -> bool {
        self.bits.iter().all(|w| *w == 0)
    }

    /// Sets a bit to 1.
    pub fn set(&mut self, index: usize) {
        let word = index / 64;
        let bit = index % 64;
        self.bits[word] |= 1 << bit;
    }

    /// Sets a bit to 0.
    pub fn unset(&mut self, index: usize) {
        let word = index / 64;
        let bit = index % 64;
        self.bits[word] &= !(1 << bit);
    }

    /// Counts the amount of bits set to 1 in this bitset.
    pub fn count_ones(&self) -> u32 {
        self.bits.iter().map(|w| w.count_ones()).sum()
    }

    // Whether `other` is a subset of `self`. This is faster than intersecting and then comparing
    // because this method short-circuits.
    pub fn contains(&self, other: &Self) -> bool {
        self.bits
            .iter()
            .zip(other.bits.iter())
            .all(|(a, b)| a & b == *b)
    }

    /// Whether `self` and `other` are disjoint.
    /// I.e. if `self` contains component A then `other` does not and vice versa.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.bits
            .iter()
            .zip(other.bits.iter())
            .all(|(a, b)| a & b == 0)
    }
}
