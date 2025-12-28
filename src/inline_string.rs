use std::{
  borrow::Borrow,
  cmp::Ordering,
  fmt::Display,
  hash::{Hash, Hasher},
};

use crate::str_cmp_x86::inline_str_eq_foreign_str;

const MAX_STRING_LEN: usize = 50;
const STRING_STORAGE_LEN: usize = 52;
const INLINE_STRING_SIZE: usize = std::mem::size_of::<InlineString>();

#[derive(Clone)]
#[repr(C, align(8))]
pub struct InlineString {
  bytes: [u8; STRING_STORAGE_LEN],
  len: u32,
}

impl InlineString {
  pub fn new(contents: &str) -> Self {
    let mut s = Self::default();
    s.initialize(contents);
    s
  }

  pub fn is_default(&self) -> bool {
    self.len == 0
  }

  pub fn len(&self) -> usize {
    self.len as usize
  }

  pub fn initialize(&mut self, contents: &str) {
    debug_assert!(
      contents.len() <= MAX_STRING_LEN,
      "{} > {}",
      contents.len(),
      MAX_STRING_LEN
    );
    // TODO: see if I can avoid the memcpy call
    unsafe { self.bytes.get_unchecked_mut(..contents.len()) }.copy_from_slice(contents.as_bytes());
    self.len = contents.len() as u32;
  }

  pub fn value_str(&self) -> &str {
    unsafe { str::from_utf8_unchecked(self.value()) }
  }

  fn value(&self) -> &[u8] {
    unsafe { self.bytes.get_unchecked(..self.len as usize) }
  }

  fn cmp_slice(&self) -> &[u8; INLINE_STRING_SIZE] {
    unsafe { &*(self as *const Self as *const [u8; INLINE_STRING_SIZE]) }
  }

  pub fn eq_foreign_str(&self, other: &str) -> bool {
    inline_str_eq_foreign_str(self, other)
  }
}

impl Default for InlineString {
  fn default() -> Self {
    Self {
      bytes: [0; STRING_STORAGE_LEN],
      len: 0,
    }
  }
}

impl PartialEq for InlineString {
  fn eq(&self, other: &Self) -> bool {
    self.cmp_slice() == other.cmp_slice()
  }
}

impl Eq for InlineString {}

impl PartialOrd for InlineString {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for InlineString {
  fn cmp(&self, other: &Self) -> Ordering {
    self.value().cmp(other.value())
  }
}

impl Borrow<[u8]> for InlineString {
  fn borrow(&self) -> &[u8] {
    self.value()
  }
}

impl Hash for InlineString {
  fn hash<H: Hasher>(&self, state: &mut H) {
    state.write(self.value());
  }
}

impl Display for InlineString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.value_str())
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cmp::Ordering,
    hash::{BuildHasher, Hasher},
  };

  use googletest::{expect_that, gtest, prelude::*};

  use crate::str_hash::BuildStringHash;

  use super::InlineString;

  #[gtest]
  fn test_construction() {
    let str1 = "testabcd";
    let i = InlineString::new(str::from_utf8(&str1.as_bytes()[..4]).unwrap());
    expect_eq!(i.value_str(), "test");
  }

  #[gtest]
  fn test_cmp() {
    let str1 = "testabcd";
    let str2 = "test1234";
    let i1 = InlineString::new(str::from_utf8(&str1.as_bytes()[..4]).unwrap());
    let i2 = InlineString::new(str::from_utf8(&str2.as_bytes()[..4]).unwrap());
    expect_true!(i1 == i2);
    expect_that!(i1.cmp(&i2), pat![Ordering::Equal]);
    expect_eq!(BuildStringHash.hash_one(&i1), BuildStringHash.hash_one(&i2));
  }

  #[gtest]
  fn test_cmp_ne_diff_length() {
    let str1 = "testabcd";
    let i1 = InlineString::new(str::from_utf8(&str1.as_bytes()[..4]).unwrap());
    let i2 = InlineString::new(str::from_utf8(&str1.as_bytes()[..5]).unwrap());
    expect_true!(i1 != i2);
    expect_that!(i1.cmp(&i2), pat![Ordering::Less]);
    expect_ne!(BuildStringHash.hash_one(&i1), BuildStringHash.hash_one(&i2));
  }

  #[gtest]
  fn test_cmp_ne_chars() {
    let str1 = "test";
    let str2 = "tesy";
    let i1 = InlineString::new(str1);
    let i2 = InlineString::new(str2);
    expect_true!(i1 != i2);
    expect_that!(i1.cmp(&i2), pat![Ordering::Less]);
    expect_ne!(BuildStringHash.hash_one(&i1), BuildStringHash.hash_one(&i2));
  }

  #[gtest]
  fn test_eq_hash_with_u8_slice() {
    let mut u8_hash = BuildStringHash.build_hasher();
    u8_hash.write("word;".as_bytes());
    expect_eq!(
      BuildStringHash.hash_one(InlineString::new("word")),
      u8_hash.finish()
    );
  }
}
