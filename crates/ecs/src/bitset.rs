use smallvec::SmallVec;

const INLINE_COUNT: usize = 4;

#[derive(Debug, Clone, Default, Hash, PartialEq, Eq)]
pub struct BitSet {
    bits: SmallVec<[u64; INLINE_COUNT]>
}

impl BitSet {
    pub fn new() -> BitSet {
        BitSet::default()
    }

    /// Returns the amount of words in this bitset.
    pub fn word_len(&self) -> usize {
        self.bits.len()
    }

    pub fn with_capacity(cap: usize) -> BitSet {
        BitSet { bits: SmallVec::with_capacity(cap) }
    }

    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    pub fn set(&mut self, index: usize) {
        let word = index / 64;
        if self.bits.len() <= word {
            self.bits.resize(word + 1, 0);
        }

        let bit = index % 64;
        self.bits[word] |= 1 << bit;
    }

    pub fn unset(&mut self, index: usize) {
        let word = index / 64;
        if self.bits.len() <= word {
            // Outside bitset, will already be 0 when accessed.
            return
        }

        let bit = index % 64;
        self.bits[word] &= !(1 << bit);
    }

    pub fn ones(&self) -> u32 {
        self.bits
            .iter()
            .map(|w| w.count_ones())
            .sum()
    }

    // Whether `other` is a subset of `self`. This is faster than intersecting and then comparing
    // because this method short-circuits.
    pub fn is_subset(&self, other: &Self) -> bool {
        if other.word_len() > self.word_len() {
            // `other` cannot be a subset of `self` if it has more words.
            if other.bits[self.bits.len()..].iter().any(|&b| b != 0) {
                return false
            }
        }

        self.bits
            .iter()
            .zip(other.bits.iter())
            .all(|(a, b)| {
                a & b == *b
            })
    }

    /// Whether `self` and `other` are disjoint.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.bits
            .iter()
            .zip(other.bits.iter())
            .all(|(a, b)| {
                a ^ b == 0
            })
    }
}