use std::{
  borrow::Borrow,
  cmp::Ordering,
  fmt::Display,
  hash::{Hash, Hasher},
  slice,
};

#[cfg(target_feature = "avx2")]
use crate::str_cmp_x86::inline_str_eq_foreign_str;

const MAX_STRING_LEN: usize = 50;
const STRING_STORAGE_LEN: usize = 12;

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
    let len = contents.len();
    debug_assert!(len <= MAX_STRING_LEN, "{len} > {MAX_STRING_LEN}");
    if len <= STRING_STORAGE_LEN {
      unsafe { self.bytes.get_unchecked_mut(..len) }.copy_from_slice(contents.as_bytes());
    } else {
      let contents = contents.to_owned().leak().as_ptr();
      unsafe { *(self.bytes.as_mut_ptr() as *mut *const u8) = contents };
    }
    self.len = len as u32;
  }

  fn is_sso(&self) -> bool {
    self.len() <= STRING_STORAGE_LEN
  }

  fn sso_value(&self) -> &[u8] {
    debug_assert!(self.is_sso());
    unsafe { self.bytes.get_unchecked(..self.len()) }
  }

  fn heap_value(&self) -> &[u8] {
    debug_assert!(!self.is_sso());
    unsafe {
      let ptr = *(self.bytes.as_ptr() as *const *const u8);
      slice::from_raw_parts(ptr, self.len())
    }
  }

  fn value(&self) -> &[u8] {
    if self.is_sso() {
      self.sso_value()
    } else {
      self.heap_value()
    }
  }

  pub fn value_str(&self) -> &str {
    unsafe { str::from_utf8_unchecked(self.value()) }
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
    self.value_str() == other.value_str()
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
