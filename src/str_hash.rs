use std::hash::{BuildHasher, Hasher};

#[derive(Default)]
pub struct BuildStringHash;

impl BuildHasher for BuildStringHash {
  type Hasher = StringHash;

  fn build_hasher(&self) -> StringHash {
    StringHash(0)
  }
}

pub struct StringHash(u64);

impl Hasher for StringHash {
  fn write(&mut self, bytes: &[u8]) {
    debug_assert_eq!(self.0, 0);
    self.0 = bytes
      .iter()
      .map(|b| *b as u64)
      .zip(std::iter::successors(Some(1u64), |acc| {
        Some(acc.wrapping_mul(1_000_000_007))
      }))
      .map(|(b, p)| b.wrapping_mul(p))
      .fold(0, |acc, h| acc.wrapping_add(h))
  }

  fn finish(&self) -> u64 {
    self.0
  }
}
