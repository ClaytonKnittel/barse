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

fn unaligned_read_would_cross_page_boundary<const READ_SIZE: usize>(start_ptr: *const u8) -> bool {
  const PAGE_SIZE: usize = 4096;
  (start_ptr as usize) % PAGE_SIZE > PAGE_SIZE - READ_SIZE
}

pub fn unaligned_u64_read_would_cross_page_boundary(start_ptr: *const u8) -> bool {
  unaligned_read_would_cross_page_boundary::<8>(start_ptr)
}

#[cfg(target_feature = "avx2")]
pub fn unaligned_m256i_read_would_cross_page_boundary(start_ptr: *const u8) -> bool {
  unaligned_read_would_cross_page_boundary::<32>(start_ptr)
}
