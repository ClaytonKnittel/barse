pub const HUGEPAGE_SIZE: usize = 2 * 1024 * 1024;

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
