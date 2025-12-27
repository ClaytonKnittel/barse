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

pub fn unaligned_u64_read_would_cross_page_boundary(start_ptr: *const u8) -> bool {
  const PAGE_SIZE: usize = 4096;
  (start_ptr as usize) % PAGE_SIZE > PAGE_SIZE - (u64::BITS as usize / 8)
}
