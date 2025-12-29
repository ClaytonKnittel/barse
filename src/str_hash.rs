use std::hash::{BuildHasher, Hasher};

#[derive(Default)]
pub struct BuildStringHash;

impl BuildHasher for BuildStringHash {
  type Hasher = StringHash;

  fn build_hasher(&self) -> StringHash {
    StringHash(0)
  }
}

#[cfg(any(test, not(target_feature = "avx2")))]
mod generic_hasher {
  use std::ptr::read_unaligned;

  use crate::util::{unaligned_read_would_cross_page_boundary, unlikely};

  fn read_str_to_u128_slow(s: &[u8]) -> u128 {
    s.iter()
      .take(16)
      .enumerate()
      .map(|(i, b)| (*b as u128) << (8 * i))
      .sum()
  }

  /// Finds the first occurrence of byte `NEEDLE` in `v`, and returns `v` with
  /// that byte and all higher-order bytes zeroed out.
  fn mask_char_and_above<const NEEDLE: u8>(v: u128) -> u128 {
    const LSB: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;
    let search_mask = (NEEDLE as u128) * LSB;
    let zeroed_needles = v ^ search_mask;
    let lsb_one_for_zeros = ((!zeroed_needles & zeroed_needles.wrapping_sub(LSB)) >> 7) & LSB;
    let keep_mask = lsb_one_for_zeros.wrapping_sub(1) & !lsb_one_for_zeros;
    v & keep_mask
  }

  fn compress_u128_to_u64(v: u128) -> u64 {
    v as u64 ^ (v >> 64) as u64
  }

  fn scramble_u64(v: u64) -> u64 {
    const MAGIC: u64 = 0x20000400020001;
    v.wrapping_mul(MAGIC) >> 48
  }

  pub fn str_hash(bytes: &[u8]) -> u64 {
    let ptr = bytes.as_ptr();
    let v = if unlikely(unaligned_read_would_cross_page_boundary::<u128>(ptr)) {
      read_str_to_u128_slow(bytes)
    } else {
      unsafe { read_unaligned(ptr as *const u128) }
    };

    let v = mask_char_and_above::<b';'>(v);
    let v = compress_u128_to_u64(v);
    scramble_u64(v)
  }

  #[cfg(test)]
  mod tests {
    use googletest::prelude::*;

    use crate::str_hash::generic_hasher::mask_char_and_above;

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
  }
}

pub struct StringHash(u64);

impl Hasher for StringHash {
  #[cfg(target_feature = "avx2")]
  fn write(&mut self, bytes: &[u8]) {
    debug_assert_eq!(self.0, 0);
    self.0 = crate::str_hash_x86::str_hash_fast(bytes);
  }

  #[cfg(not(target_feature = "avx2"))]
  fn write(&mut self, bytes: &[u8]) {
    debug_assert_eq!(self.0, 0);
    self.0 = generic_hasher::str_hash(bytes);
  }

  fn write_u8(&mut self, _: u8) {
    unimplemented!();
  }

  fn finish(&self) -> u64 {
    self.0
  }
}

#[cfg(test)]
mod tests {
  use std::hash::{BuildHasher, Hasher};

  use googletest::prelude::*;
  use itertools::Itertools;
  use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
    Rng, SeedableRng,
  };

  use crate::str_hash::{generic_hasher, BuildStringHash};

  fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = BuildStringHash.build_hasher();
    hasher.write(bytes);
    hasher.finish()
  }

  #[gtest]
  fn test_str_hash_different_positions() {
    #[repr(align(4096))]
    struct PageAligned([u8; 8192]);

    let s = b"test;123";
    let mut page_aligned = PageAligned([0xa4; 8192]);
    // Aligned load
    page_aligned.0[0..8].copy_from_slice(s);
    // Cross cache line
    page_aligned.0[60..68].copy_from_slice(s);
    // Cross page boundary
    page_aligned.0[4093..4101].copy_from_slice(s);

    let expected_hash = hash_bytes(&"test;123".as_bytes()[0..4]);
    expect_eq!(hash_bytes(&page_aligned.0[0..4]), expected_hash);
    expect_eq!(hash_bytes(&page_aligned.0[60..64]), expected_hash);
    expect_eq!(hash_bytes(&page_aligned.0[4093..4097]), expected_hash);
  }

  #[gtest]
  fn test_str_hash_fuzz() {
    let mut rng = StdRng::seed_from_u64(0x4214931);
    let distr = Uniform::new(2, 50).unwrap();

    fn rand_u8_excluding_semicolon<R: Rng>(rng: &mut R) -> u8 {
      let distr = Uniform::new(0, 254).unwrap();
      let v = distr.sample(rng);
      if v >= b';' {
        v + 1
      } else {
        v
      }
    }

    for _ in 0..10 {
      let rand_len = distr.sample(&mut rng);
      let str_bytes = (0..rand_len)
        .map(|_| rand_u8_excluding_semicolon(&mut rng))
        .chain(std::iter::once(b';'))
        .collect_vec();

      let fast_hash = hash_bytes(&str_bytes[..rand_len]);
      let slow_hash = generic_hasher::str_hash(&str_bytes[..rand_len]);
      assert_eq!(fast_hash, slow_hash);
    }
  }
}
