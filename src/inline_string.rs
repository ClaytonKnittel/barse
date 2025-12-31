use std::{borrow::Borrow, cmp::Ordering, fmt::Display};

#[cfg(target_feature = "avx2")]
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

  /// Performs a memcpy from contents to self.value() without calling
  /// libc::memcpy.
  fn memcpy_no_libc(&mut self, contents: &str) {
    for i in 0..contents.len().min(MAX_STRING_LEN) {
      unsafe {
        *self.bytes.get_unchecked_mut(i) =
          std::hint::black_box(*contents.as_bytes().get_unchecked(i));
      }
    }
  }

  pub fn initialize(&mut self, contents: &str) {
    debug_assert!(
      contents.len() <= MAX_STRING_LEN,
      "{} > {}",
      contents.len(),
      MAX_STRING_LEN
    );
    // TODO: see if I can avoid the memcpy call
    self.memcpy_no_libc(contents);
    self.len = contents.len() as u32;
  }

  pub fn value_str(&self) -> &str {
    unsafe { str::from_utf8_unchecked(self.value()) }
  }

  pub fn value(&self) -> &[u8] {
    unsafe { self.bytes.get_unchecked(..self.len as usize) }
  }

  fn cmp_slice(&self) -> &[u8; INLINE_STRING_SIZE] {
    unsafe { &*(self as *const Self as *const [u8; INLINE_STRING_SIZE]) }
  }

  #[cfg(target_feature = "avx2")]
  pub fn eq_foreign_str(&self, other: &str) -> bool {
    inline_str_eq_foreign_str(self, other)
  }

  #[cfg(not(target_feature = "avx2"))]
  pub fn eq_foreign_str(&self, other: &str) -> bool {
    self.value_str() == other
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

impl Display for InlineString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.value_str())
  }
}

#[cfg(test)]
mod tests {
  use std::cmp::Ordering;

  use googletest::{expect_that, gtest, prelude::*};

  use crate::str_hash::str_hash;

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
    expect_eq!(str_hash(&i1.bytes), str_hash(&i2.bytes));
  }

  #[gtest]
  fn test_cmp_ne_diff_length() {
    let str1 = "testabcd";
    let i1 = InlineString::new(str::from_utf8(&str1.as_bytes()[..4]).unwrap());
    let i2 = InlineString::new(str::from_utf8(&str1.as_bytes()[..5]).unwrap());
    expect_true!(i1 != i2);
    expect_that!(i1.cmp(&i2), pat![Ordering::Less]);
    expect_ne!(str_hash(&i1.bytes), str_hash(&i2.bytes));
  }

  #[gtest]
  fn test_cmp_ne_chars() {
    let str1 = "test";
    let str2 = "tesy";
    let i1 = InlineString::new(str1);
    let i2 = InlineString::new(str2);
    expect_true!(i1 != i2);
    expect_that!(i1.cmp(&i2), pat![Ordering::Less]);
    expect_ne!(str_hash(&i1.bytes), str_hash(&i2.bytes));
  }

  #[gtest]
  fn test_eq_hash_with_u8_slice() {
    expect_eq!(
      str_hash(&InlineString::new("word").bytes),
      str_hash("word".as_bytes())
    );
  }
}
