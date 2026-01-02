use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};
use std::{borrow::Borrow, cell::UnsafeCell, cmp::Ordering, fmt::Display};

#[cfg(target_feature = "avx2")]
use crate::str_cmp_x86::inline_str_eq_foreign_str;
use crate::util::{likely, InPlaceInitializable};

const MAX_STRING_LEN: usize = 50;
const STRING_STORAGE_LEN: usize = 52;
const INLINE_STRING_SIZE: usize = std::mem::size_of::<InlineString>();

#[repr(C, align(8))]
pub struct InlineString {
  bytes: UnsafeCell<[u8; STRING_STORAGE_LEN]>,
  len: AtomicU32,
}

impl InlineString {
  const INITIALIZING_RESERVED_LEN: u32 = u32::MAX;

  #[cfg(test)]
  pub fn new(contents: &str) -> Self {
    let s = Self::default();
    Self::memcpy_no_libc(unsafe { &mut *s.bytes.get() }, contents);
    s.len.store(contents.len() as u32, AtomicOrdering::Relaxed);
    s
  }

  fn bytes(&self) -> &[u8; STRING_STORAGE_LEN] {
    unsafe { &*self.bytes.get() }
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn len(&self) -> usize {
    self.len.load(AtomicOrdering::Relaxed) as usize
  }

  /// Performs a memcpy from contents to self.value() without calling
  /// libc::memcpy.
  fn memcpy_no_libc(bytes: &mut [u8], contents: &str) {
    for i in 0..contents.len().min(MAX_STRING_LEN) {
      unsafe {
        *bytes.get_unchecked_mut(i) = std::hint::black_box(*contents.as_bytes().get_unchecked(i));
      }
    }
  }

  pub fn initialized(&self) -> bool {
    let len = self.len.load(AtomicOrdering::Acquire);
    len != 0 && len != Self::INITIALIZING_RESERVED_LEN
  }

  pub fn value_str(&self) -> &str {
    unsafe { str::from_utf8_unchecked(self.value()) }
  }

  pub fn value(&self) -> &[u8] {
    unsafe { self.bytes().get_unchecked(..self.len()) }
  }

  fn cmp_slice(&self) -> &[u8; INLINE_STRING_SIZE] {
    unsafe { &*(self as *const Self as *const [u8; INLINE_STRING_SIZE]) }
  }

  fn memcpy_no_libc_under_lock(&self, contents: &str) {
    debug_assert_eq!(self.len() as u32, Self::INITIALIZING_RESERVED_LEN);
    Self::memcpy_no_libc(unsafe { &mut *self.bytes.get() }, contents);
  }

  fn initialize_contents_under_lock(&self, contents: &str) {
    debug_assert!(contents.len() <= MAX_STRING_LEN,);
    self.memcpy_no_libc_under_lock(contents);
  }

  #[cfg(target_feature = "avx2")]
  fn eq_foreign_str(&self, other: &str) -> bool {
    debug_assert!(self.initialized());
    inline_str_eq_foreign_str(self, other)
  }

  #[cfg(not(target_feature = "avx2"))]
  fn eq_foreign_str(&self, other: &str) -> bool {
    debug_assert!(self.initialized());
    self.value_str() == other
  }

  fn wait_until_initialized(&self) {
    while self.len.load(AtomicOrdering::Acquire) == Self::INITIALIZING_RESERVED_LEN {
      std::hint::spin_loop();
    }
  }

  pub fn eq_or_initialize(&self, station: &str) -> bool {
    if likely(self.initialized()) {
      return likely(self.eq_foreign_str(station));
    }

    let prev_len = self
      .len
      .swap(Self::INITIALIZING_RESERVED_LEN, AtomicOrdering::Acquire);
    if prev_len == 0 {
      self.initialize_contents_under_lock(station);
      self
        .len
        .store(station.len() as u32, AtomicOrdering::Release);
      return true;
    } else if prev_len == Self::INITIALIZING_RESERVED_LEN {
      self.wait_until_initialized();
    } else {
      // We accidentally overwrite the length with INITIALIZING_RESERVED_LEN,
      // restore the length:
      self.len.store(prev_len, AtomicOrdering::Relaxed);
    }

    self.eq_foreign_str(station)
  }
}

impl Default for InlineString {
  fn default() -> Self {
    Self {
      bytes: UnsafeCell::new([0; STRING_STORAGE_LEN]),
      #[cfg(feature = "multithreaded")]
      len: AtomicU32::new(0),
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

impl InPlaceInitializable for InlineString {
  fn initialize(&mut self) {
    // No need to do anything, a zero-initialized string is correctly initialized.
    debug_assert!(self.bytes().iter().all(|b| *b == 0));
    debug_assert_eq!(self.len(), 0);
  }
}

unsafe impl Sync for InlineString {}

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
    expect_eq!(str_hash(i1.bytes()), str_hash(i2.bytes()));
  }

  #[gtest]
  fn test_cmp_ne_diff_length() {
    let str1 = "testabcd";
    let i1 = InlineString::new(str::from_utf8(&str1.as_bytes()[..4]).unwrap());
    let i2 = InlineString::new(str::from_utf8(&str1.as_bytes()[..5]).unwrap());
    expect_true!(i1 != i2);
    expect_that!(i1.cmp(&i2), pat![Ordering::Less]);
    expect_ne!(str_hash(i1.bytes()), str_hash(i2.bytes()));
  }

  #[gtest]
  fn test_cmp_ne_chars() {
    let str1 = "test";
    let str2 = "tesy";
    let i1 = InlineString::new(str1);
    let i2 = InlineString::new(str2);
    expect_true!(i1 != i2);
    expect_that!(i1.cmp(&i2), pat![Ordering::Less]);
    expect_ne!(str_hash(i1.bytes()), str_hash(i2.bytes()));
  }

  #[gtest]
  fn test_eq_hash_with_u8_slice() {
    expect_eq!(
      str_hash(InlineString::new("word").bytes()),
      str_hash("word".as_bytes())
    );
  }
}
