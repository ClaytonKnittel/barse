use std::arch::x86_64::{
  __m256i, _mm256_cmpeq_epi8, _mm256_load_si256, _mm256_movemask_epi8, _mm256_set1_epi8,
  _mm256_store_si256,
};

#[derive(Clone, Copy)]
pub struct Cache((__m256i, __m256i));

impl Cache {
  pub const BYTES_PER_BUFFER: usize = 64;

  pub fn aligned_store(&self, ptr: *mut u8) {
    let m256_ptr = ptr as *mut __m256i;
    debug_assert!(m256_ptr.is_aligned());
    unsafe {
      _mm256_store_si256(m256_ptr, self.0 .0);
      _mm256_store_si256(m256_ptr.add(1), self.0 .1);
    }
  }

  #[target_feature(enable = "avx2")]
  fn read_next_from_buffer_avx(buffer: &[u8]) -> (Cache, u64, u64) {
    let m256_ptr = buffer.as_ptr() as *const __m256i;
    let cache1 = unsafe { _mm256_load_si256(m256_ptr) };
    let cache2 = unsafe { _mm256_load_si256(m256_ptr.add(1)) };

    let semicolon_mask =
      Self::char_mask(cache1, b';') as u64 + ((Self::char_mask(cache2, b';') as u64) << 32);
    let newline_mask =
      Self::char_mask(cache1, b'\n') as u64 + ((Self::char_mask(cache2, b'\n') as u64) << 32);
    (Cache((cache1, cache2)), semicolon_mask, newline_mask)
  }

  pub fn read_next_from_buffer(buffer: &[u8]) -> (Cache, u64, u64) {
    unsafe { Self::read_next_from_buffer_avx(buffer) }
  }

  #[target_feature(enable = "avx2")]
  fn char_mask(cache: __m256i, needle: u8) -> u32 {
    let seach_mask = _mm256_set1_epi8(needle as i8);
    let eq_mask = _mm256_cmpeq_epi8(cache, seach_mask);
    _mm256_movemask_epi8(eq_mask) as u32
  }
}
