//! Discrete Fourier Transform, in-place, decimation-in-time
//!
//! Straightforward recursive algorithm, "unrolled" up to size 256.
//!
//! Inspired by Bernstein's djbfft: https://cr.yp.to/djbfft.html

extern crate alloc;
use alloc::vec::Vec;

use itertools::izip;
use p3_field::PackedField;

use crate::{to_mersenne31_array, Mersenne31};

// the twiddle for the inner most layer is 2^16 (65536)
pub(crate) const INV_TWIDDLES_4: [Mersenne31; 2] = to_mersenne31_array([991237807, 1371835609]);
pub(crate) const INV_TWIDDLES_8: [Mersenne31; 4] = to_mersenne31_array([1160411471, 490549293, 204982243, 628957573]);
pub(crate) const INV_TWIDDLES_16: [Mersenne31; 8] = to_mersenne31_array([1899133673, 685780700, 761629115, 1798231519, 421110815, 1859156789, 655012266, 969924856]);
pub(crate) const INV_TWIDDLES_32: [Mersenne31; 16] = to_mersenne31_array([959596234, 721860568, 218253990, 1955048591, 1046725194, 1036402186, 1268642696, 559888787, 1381082522, 2138687850, 448375059, 559361447, 1356867335, 1767118503, 1051468699, 2029303208]);

impl Mersenne31 {
    #[inline(always)]
    fn backward_butterfly<PF: PackedField<Scalar = Mersenne31>>(
        x: PF,
        y: PF,
        w: Self,
    ) -> (PF, PF) {
        let t = (x - y) * PF::from_f(w); // Should use a custom field function for this.
        (x + y, t)
    }

    #[inline]
    fn backward_pass<PF: PackedField<Scalar = Mersenne31>>(a: &mut [PF], roots: &[Self]) {
        let half_n = a.len() / 2;
        assert_eq!(roots.len(), half_n - 1);

        // Safe because 0 <= half_n < a.len()
        let (top, tail) = unsafe { a.split_at_mut_unchecked(half_n) };

        izip!(top.iter_mut(), tail.iter_mut(), roots).for_each(|(hi, lo, &root)| {
            (*hi, *lo) = Self::backward_butterfly(*hi, *lo, root);
        });
    }

    #[inline(always)]
    fn backward_2<PF: PackedField<Scalar = Mersenne31>>(a: &mut [PF]) {
        assert_eq!(a.len(), 2);

        let s = a[0] + a[1];
        let t = a[0] - a[1];
        a[0] = s;
        a[1] = t.mul_2exp_u64(16); // The twiddle for the inner most layer is 2^16.
    }

    #[inline(always)]
    fn backward_4<PF: PackedField<Scalar = Mersenne31>>(a: &mut [PF]) {
        assert_eq!(a.len(), 4);

        Self::backward_pass(a, &INV_TWIDDLES_4);

        let (a0, a1) = unsafe { a.split_at_mut_unchecked(a.len() / 2) };
        Self::backward_2(a0);
        Self::backward_2(a1);

        
    }

    #[inline(always)]
    fn backward_8<PF: PackedField<Scalar = Mersenne31>>(a: &mut [PF]) {
        assert_eq!(a.len(), 8);

        // Safe because a.len() == 8
        let (a0, a1) = unsafe { a.split_at_mut_unchecked(a.len() / 2) };
        Self::backward_4(a0);
        Self::backward_4(a1);

        Self::backward_pass(a, &INV_TWIDDLES_8);
    }

    #[inline(always)]
    fn backward_16<PF: PackedField<Scalar = Mersenne31>>(a: &mut [PF]) {
        assert_eq!(a.len(), 16);

        // Safe because a.len() == 16
        let (a0, a1) = unsafe { a.split_at_mut_unchecked(a.len() / 2) };
        Self::backward_8(a0);
        Self::backward_8(a1);

        Self::backward_pass(a, &INV_TWIDDLES_16);
    }

    #[inline(always)]
    fn backward_32<PF: PackedField<Scalar = Mersenne31>>(
        a: &mut [PF],
    ) {
        assert_eq!(a.len(), 32);

        // Safe because a.len() == 32
        let (a0, a1) = unsafe { a.split_at_mut_unchecked(a.len() / 2) };
        Self::backward_16(a0);
        Self::backward_16(a1);

        Self::backward_pass(a, &INV_TWIDDLES_32);
    }

    #[inline(always)]
    fn backward_64<PF: PackedField<Scalar = Mersenne31>>(
        a: &mut [PF],
        root_table: &[Vec<Self>],
    ) {
        assert_eq!(a.len(), 64);

        // Safe because a.len() == 64
        let (a0, a1) = unsafe { a.split_at_mut_unchecked(a.len() / 2) };
        Self::backward_32(a0);
        Self::backward_32(a1);

        Self::backward_pass(a, &root_table[0]);
    }

    #[inline(always)]
    fn backward_128<PF: PackedField<Scalar = Mersenne31>>(
        a: &mut [PF],
        root_table: &[Vec<Self>],
    ) {
        assert_eq!(a.len(), 128);

        // Safe because a.len() == 128
        let (a0, a1) = unsafe { a.split_at_mut_unchecked(a.len() / 2) };
        Self::backward_64(a0, &root_table[1..]);
        Self::backward_64(a1, &root_table[1..]);

        Self::backward_pass(a, &root_table[0]);
    }

    #[inline(always)]
    fn backward_256<PF: PackedField<Scalar = Mersenne31>>(
        a: &mut [PF],
        root_table: &[Vec<Self>],
    ) {
        assert_eq!(a.len(), 256);

        // Safe because a.len() == 256
        let (a0, a1) = unsafe { a.split_at_mut_unchecked(a.len() / 2) };
        Self::backward_128(a0, &root_table[1..]);
        Self::backward_128(a1, &root_table[1..]);

        Self::backward_pass(a, &root_table[0]);
    }

    #[inline]
    pub fn backward_fft<PF: PackedField<Scalar = Mersenne31>>(
        a: &mut [PF],
        twiddle_table: &[Vec<Self>],
    ) {
        let n = a.len();
        if n == 1 {
            return;
        }

        assert_eq!(n, 1 << (twiddle_table.len() + 1));
        match n {
            256 => Self::backward_256(a, twiddle_table),
            128 => Self::backward_128(a, twiddle_table),
            64 => Self::backward_64(a, twiddle_table),
            32 => Self::backward_32(a),
            16 => Self::backward_16(a),
            8 => Self::backward_8(a),
            4 => Self::backward_4(a),
            2 => Self::backward_2(a),
            _ => {
                debug_assert!(n > 64);

                // Safe because a.len() > 64
                let (a0, a1) = unsafe { a.split_at_mut_unchecked(n / 2) };
                Self::backward_fft(a0, &twiddle_table[1..]);
                Self::backward_fft(a1, &twiddle_table[1..]);

                Self::backward_pass(a, &twiddle_table[0]);
            }
        }
    }
}