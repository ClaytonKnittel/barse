use std::{
  borrow::Borrow,
  cmp::Ordering,
  fmt::Display,
  hash::{Hash, Hasher},
};

const MAX_STRING_LEN: usize = 50;
const STRING_STORAGE_LEN: usize = 56;

#[repr(C, align(64))]
pub struct InlineString {
  bytes: [u8; STRING_STORAGE_LEN],
  len: usize,
}

impl InlineString {
  pub fn new(contents: &str) -> Self {
    let mut s = Self::default();
    s.initialize(contents);
    s
  }

  pub fn initialize(&mut self, contents: &str) {
    debug_assert!(contents.len() <= MAX_STRING_LEN);
    // TODO: see if I can avoid the memcpy call
    unsafe { self.bytes.get_unchecked_mut(..contents.len()) }.copy_from_slice(contents.as_bytes());
  }

  pub fn value(&self) -> &str {
    unsafe { str::from_utf8_unchecked(&self.bytes) }
  }

  fn full_slice(&self) -> &[u8; 64] {
    unsafe { &*(self as *const Self as *const [u8; 64]) }
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
    self.full_slice() == other.full_slice()
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
    state.write(&self.bytes);
  }
}

impl Display for InlineString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.value())
  }
}
