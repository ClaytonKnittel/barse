use std::{
  hash::{BuildHasher, Hasher},
  ptr::read_unaligned,
};

use crate::util::{unaligned_u64_read_would_cross_page_boundary, unlikely};

#[derive(Default)]
pub struct BuildStringHash;

impl BuildHasher for BuildStringHash {
  type Hasher = StringHash;

  fn build_hasher(&self) -> StringHash {
    StringHash(0)
  }
}

/// Finds the first occurrence of byte `NEEDLE` in `v`, and returns `v` with
/// that byte and all higher-order bytes zeroed out.
fn mask_char_and_above<const NEEDLE: u8>(v: u64) -> u64 {
  const LSB: u64 = 0x0101_0101_0101_0101;
  let search_mask = (NEEDLE as u64) * LSB;
  let zeroed_needles = v ^ search_mask;
  let lsb_one_for_zeros = ((!zeroed_needles & zeroed_needles.wrapping_sub(LSB)) >> 7) & LSB;
  let keep_mask = lsb_one_for_zeros.wrapping_sub(1) & !lsb_one_for_zeros;
  v & keep_mask
}

fn compress_lower_nibbles(v: u64) -> u32 {
  const LOWER_NIBBLE: u32 = 0x0f0f_0f0f;
  (v as u32 & LOWER_NIBBLE) | ((v >> 28) as u32 & !LOWER_NIBBLE)
}

fn scramble_u32(v: u32) -> u32 {
  const MAGIC: u32 = 0x01008021;
  v.wrapping_mul(MAGIC).reverse_bits()
}

pub struct StringHash(u64);

impl Hasher for StringHash {
  fn write(&mut self, bytes: &[u8]) {
    let ptr = bytes.as_ptr();
    if unlikely(unaligned_u64_read_would_cross_page_boundary(ptr)) {
      todo!();
    }

    let v = unsafe { read_unaligned(ptr as *const u64) };
    let v = mask_char_and_above::<b';'>(v);
    let v = compress_lower_nibbles(v);
    let v = scramble_u32(v);
    self.0 ^= v as u64
  }

  fn finish(&self) -> u64 {
    self.0
  }
}

#[cfg(test)]
mod tests {
  use googletest::prelude::*;

  use crate::str_hash::{compress_lower_nibbles, mask_char_and_above};

  #[gtest]
  fn test_mask_char_and_above() {
    expect_eq!(
      mask_char_and_above::<0x12>(0x10_11_12_13_14_15_16_17),
      0x00_00_00_13_14_15_16_17
    );
    expect_eq!(
      mask_char_and_above::<0x20>(0x10_11_12_13_14_15_16_17),
      0x10_11_12_13_14_15_16_17
    );
  }

  #[gtest]
  fn test_compress_lower_nibbles() {
    expect_eq!(
      compress_lower_nibbles(0x51_62_73_84_ab_cd_ef_09),
      0x1b_2d_3f_49
    );
  }
}
