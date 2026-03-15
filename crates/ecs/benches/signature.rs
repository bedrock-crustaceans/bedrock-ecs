use std::arch::x86_64::{
    __m128i, __m256i, _mm_load_si128, _mm256_and_si256, _mm256_andnot_si256, _mm256_load_si256,
    _mm256_loadu_ps, _mm256_loadu_si256, _mm256_set_epi64x, _mm256_testz_si256,
};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

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
        // if is_x86_feature_detected!("sse4.1") {
        //     // Safety: This is safe to call because by the conditional above, the `avx2` feature
        //     // is available.
        //     return unsafe { self.contains_avx(other) }
        // }

        self.bits
            .iter()
            .zip(other.bits.iter())
            .all(|(a, b)| a & b == *b)
    }

    #[inline]
    #[target_feature(enable = "sse4.1")]
    pub fn contains_sse(&self, other: &Self) -> bool {
        use std::arch::x86_64::{__m128i, _mm_andnot_si128, _mm_loadu_si128, _mm_testc_si128};

        let va = unsafe { _mm_loadu_si128(self.bits.as_ptr().cast::<__m128i>()) };
        let vb = unsafe { _mm_loadu_si128(other.bits.as_ptr().cast::<__m128i>()) };

        _mm_testc_si128(va, vb) != 0
    }

    /// Whether `self` and `other` are disjoint.
    /// I.e. if `self` contains component A then `other` does not and vice versa.
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.bits
            .iter()
            .zip(other.bits.iter())
            .all(|(a, b)| a & b == 0)
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    fn is_disjoint_avx(&self, other: &Self) -> bool {
        use std::arch::x86_64::{
            __m256i, _mm256_andnot_si256, _mm256_loadu_si256, _mm256_testz_si256,
        };

        let len = self.bits.len().min(other.bits.len());
        let mut i = 0;

        while i + 4 <= len {
            let va = unsafe { _mm256_loadu_si256(self.bits.as_ptr().add(i).cast::<__m256i>()) };
            let vb = unsafe { _mm256_loadu_si256(other.bits.as_ptr().add(i).cast::<__m256i>()) };

            let diff = _mm256_andnot_si256(va, vb);
            if unsafe { _mm256_testz_si256(diff, diff) } == 0 {
                return false;
            }

            i += 4;
        }

        while i < len {
            if (other.bits[i]) & (!self.bits[i]) != 0 {
                return false;
            }
            i += 1;
        }

        true
    }
}

fn signature_contains(bitset: &(Signature, Signature)) {
    assert!(bitset.0.contains(&bitset.1));
}

#[target_feature(enable = "sse4.1")]
fn signature_contains_simd128(bitset: &(Signature, Signature)) {
    assert!(bitset.0.contains_sse(&bitset.1));
}

fn bitset_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitset_single");

    for size in [1, 2, 3, 4] {
        let mut sig = Signature::new();
        for i in (0..64 * size).step_by(2) {
            sig.set(i);
        }

        let mut sig2 = Signature::new();
        for i in (0..64 * size).step_by(4) {
            sig2.set(i);
        }

        group.throughput(Throughput::Bits((size * 64) as u64));
        group.bench_with_input(
            BenchmarkId::new("signature_contains", size * 64),
            &(sig.clone(), sig2.clone()),
            |b, i| b.iter(|| signature_contains(i)),
        );

        group.throughput(Throughput::Bits((size * 64) as u64));
        group.bench_with_input(
            BenchmarkId::new("signature_contains_simd128", size * 64),
            &(sig, sig2),
            |b, i| b.iter(|| unsafe { signature_contains_simd128(i) }),
        );
    }
    group.finish();
}

criterion_group!(benches, bitset_benchmark);
criterion_main!(benches);
