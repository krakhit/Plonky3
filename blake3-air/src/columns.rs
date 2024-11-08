use core::borrow::{Borrow, BorrowMut};
use core::mem::size_of;

use crate::constants::U32_LIMBS;

/// Columns for a Blake-3 AIR which computes one permutation per row.
///
/// This is a pretty wide trace but that should be fine.
#[repr(C)]
pub struct Blake3Cols<T> {
    // The inputs to the hash function.
    pub inputs: [[T; 32]; 16],

    // The chaining values are the first eight outputs of the previous compression.
    pub chaining_values: [[[T; 32]; 4]; 2],

    // A few auxillary values use to flesh out the first state.
    pub counter_low: [T; 32],
    pub counter_hi: [T; 32],
    pub block_len: [T; 32],
    pub flags: [T; 32],

    // It should be possible to remove these two but this makes a negligible difference to the overall width of the trace.
    pub initial_row0: [[T; U32_LIMBS]; 4],
    pub initial_row2: [[T; U32_LIMBS]; 4],

    pub full_rounds: [FullRound<T>; 7],

    pub final_round_helpers: [[T; 32]; 4],

    pub outputs: [[[T; 32]; 4]; 4],
}

/// A state at a single instance of time.
///
/// Rows `0` and `2` are saved as `2` `16` bit limbs.
/// Rows `1` and `3` are saved as `32` boolean values.
#[repr(C)]
pub struct Blake3State<T> {
    pub row0: [[T; U32_LIMBS]; 4],
    pub row1: [[T; 32]; 4],
    pub row2: [[T; U32_LIMBS]; 4],
    pub row3: [[T; 32]; 4],
}

/// Full round columns.
#[repr(C)]
pub struct FullRound<T> {
    // A full round of the Blake3 hash consists of 8 applications of the mixing function.
    // the first four mixing functions act on the four columns and the second four
    // functions act on the diagonals.

    // We use the output of the previous row to get the input to this row.
    /// The outputs after applying the first half of the first 4 mixing functions
    pub state_prime: Blake3State<T>,

    /// The helper values for the summations in the mixing functions operating on the columns.
    /// The inner [T; 2] gives the number of overflows working mod 2^32 and 2^16 respectively.
    pub aux_columns: [[[T; 2]; 4]; 4],

    /// The outputs after applying the first 4 row-mixing functions.
    pub state_middle: Blake3State<T>,

    /// The outputs after applying the first half of the diagonal mixing functions.
    pub state_middle_prime: Blake3State<T>,

    /// The helper values for the summations in the mixing functions operating on the diagonals.
    pub aux_diagonals: [[[T; 2]; 4]; 4],

    /// This will also be the input to the next row.
    pub state_output: Blake3State<T>,
}

/// Data needed to verify a single QuarterRound mixing function.
#[repr(C)]
pub(crate) struct QuarterRound<'a, T, U> {
    // A full round of the Blake3 hash consists of 8 applications of the mixing function.
    pub a: &'a [T; U32_LIMBS],
    pub b: &'a [T; 32],
    pub c: &'a [T; U32_LIMBS],
    pub d: &'a [T; 32],

    pub m_two_i: &'a [U; U32_LIMBS], // m_{2i}
    pub sum_1_aux: &'a [T; 2], // Auxillary variables used to verify a'  = a + b + m_{2i}  mod 2^32
    pub sum_2_aux: &'a [T; 2], // Auxillary variables used to verify c'  = c + d mod 2^32

    pub a_prime: &'a [T; U32_LIMBS],
    pub b_prime: &'a [T; 32],
    pub c_prime: &'a [T; U32_LIMBS],
    pub d_prime: &'a [T; 32],

    pub m_two_i_plus_one: &'a [U; U32_LIMBS], // m_{2i + 1}
    pub sum_3_aux: &'a [T; 2], // Auxillary variables used to verify a_output = a' + b' + m_{2i + 1}  mod 2^32
    pub sum_4_aux: &'a [T; 2], // Auxillary variables used to verify c_output = c' + d' mod 2^32

    pub a_output: &'a [T; U32_LIMBS],
    pub b_output: &'a [T; 32],
    pub c_output: &'a [T; U32_LIMBS],
    pub d_output: &'a [T; 32],
}

pub const NUM_BLAKE3_COLS: usize = size_of::<Blake3Cols<u8>>();

impl<T> Borrow<Blake3Cols<T>> for [T] {
    fn borrow(&self) -> &Blake3Cols<T> {
        debug_assert_eq!(self.len(), NUM_BLAKE3_COLS);
        let (prefix, shorts, suffix) = unsafe { self.align_to::<Blake3Cols<T>>() };
        debug_assert!(prefix.is_empty(), "Alignment should match");
        debug_assert!(suffix.is_empty(), "Alignment should match");
        debug_assert_eq!(shorts.len(), 1);
        &shorts[0]
    }
}

impl<T> BorrowMut<Blake3Cols<T>> for [T] {
    fn borrow_mut(&mut self) -> &mut Blake3Cols<T> {
        debug_assert_eq!(self.len(), NUM_BLAKE3_COLS);
        let (prefix, shorts, suffix) = unsafe { self.align_to_mut::<Blake3Cols<T>>() };
        debug_assert!(prefix.is_empty(), "Alignment should match");
        debug_assert!(suffix.is_empty(), "Alignment should match");
        debug_assert_eq!(shorts.len(), 1);
        &mut shorts[0]
    }
}
