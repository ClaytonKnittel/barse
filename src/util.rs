#[inline(always)]
#[cold]
fn cold_path() {}

#[inline(always)]
pub fn likely(b: bool) -> bool {
  if b {
    true
  } else {
    cold_path();
    false
  }
}

#[inline(always)]
pub fn unlikely(b: bool) -> bool {
  if b {
    cold_path();
    true
  } else {
    false
  }
}

pub fn unaligned_read_would_cross_page_boundary<T>(start_ptr: *const u8) -> bool {
  const PAGE_SIZE: usize = 4096;
  (start_ptr as usize) % PAGE_SIZE > PAGE_SIZE - std::mem::size_of::<T>()
}

pub trait HasIter<'a> {
  type Item: 'a;

  fn iter(&'a self) -> impl Iterator<Item = Self::Item>;
}

pub trait BitVector {
  /// Returns the index of the least-significant 1-bit, and clears that bit
  /// from `self`. Expects `self != 0`.
  fn pop_lsb(&mut self) -> u32;
}

impl BitVector for u64 {
  fn pop_lsb(&mut self) -> u32 {
    debug_assert!(*self != 0);
    if *self == 0 {
      unsafe { std::hint::unreachable_unchecked() };
    }
    let offset = self.trailing_zeros();
    *self &= *self - 1;
    offset
  }
}
