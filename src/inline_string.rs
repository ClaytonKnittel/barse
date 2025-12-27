use std::{
  borrow::Borrow,
  cmp::Ordering,
  fmt::Display,
  hash::{Hash, Hasher},
};

const MAX_STRING_LEN: usize = 6;
const STRING_STORAGE_LEN: usize = 8;
const INLINE_STRING_SIZE: usize = std::mem::size_of::<InlineString>();

#[repr(C, align(8))]
pub struct InlineString {
  len: usize,
  bytes: [u8; STRING_STORAGE_LEN],
}

impl InlineString {
  pub fn new(contents: &str) -> Self {
    let mut s = Self::default();
    s.initialize(contents);
    s
  }

  pub fn initialize(&mut self, contents: &str) {
    debug_assert!(
      contents.len() <= MAX_STRING_LEN,
      "{} > {}",
      contents.len(),
      MAX_STRING_LEN
    );
    self.len = contents.len();
    // TODO: see if I can avoid the memcpy call
    unsafe { self.bytes.get_unchecked_mut(..contents.len()) }.copy_from_slice(contents.as_bytes());
  }

  pub fn value(&self) -> &str {
    unsafe { str::from_utf8_unchecked(self.bytes.get_unchecked(..self.len)) }
  }

  fn cmp_slice(&self) -> &[u8] {
    unsafe { &*(self as *const Self as *const [u8; INLINE_STRING_SIZE]) }
  }
}

impl Default for InlineString {
  fn default() -> Self {
    Self {
      len: 0,
      bytes: [0; STRING_STORAGE_LEN],
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

impl Borrow<str> for InlineString {
  fn borrow(&self) -> &str {
    self.value()
  }
}

impl Hash for InlineString {
  fn hash<H: Hasher>(&self, state: &mut H) {
    debug_assert!(self.bytes[self.len..].iter().all(|b| *b == 0));
    state.write(self.value().as_bytes());
  }
}

impl Display for InlineString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.value())
  }
}

#[cfg(test)]
mod tests {
  use std::cmp::Ordering;

  use googletest::{expect_that, gtest, prelude::*};

  use super::InlineString;

  #[gtest]
  fn test_cmp() {
    let str1 = "testabcd";
    let str2 = "test1234";
    let i1 = InlineString::new(str::from_utf8(&str1.as_bytes()[..4]).unwrap());
    let i2 = InlineString::new(str::from_utf8(&str2.as_bytes()[..4]).unwrap());
    expect_true!(i1 == i2);
    expect_that!(i1.cmp(&i2), pat![Ordering::Equal]);
  }
}
