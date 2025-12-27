use std::arch::x86_64::{
  __m256i, _mm256_cmpeq_epi8, _mm256_load_si256, _mm256_movemask_epi8, _mm256_set1_epi8,
  _mm256_store_si256,
};

#[derive(Clone, Copy)]
pub struct Cache(__m256i);

impl Cache {
  pub const fn bytes_per_buffer() -> usize {
    32
  }

  pub fn aligned_store(&self, ptr: *mut u8) {
    let m256_ptr = ptr as *mut __m256i;
    debug_assert!(m256_ptr.is_aligned());
    unsafe { _mm256_store_si256(m256_ptr, self.0) };
  }

  #[target_feature(enable = "avx2")]
  pub fn read_next_from_buffer(buffer: &[u8]) -> (Cache, u32, u32) {
    let cache = unsafe { _mm256_load_si256(buffer.as_ptr() as *const __m256i) };
    let semicolon_mask = Self::char_mask(cache, b';');
    let newline_mask = Self::char_mask(cache, b'\n');
    (Cache(cache), semicolon_mask, newline_mask)
  }

  #[target_feature(enable = "avx2")]
  fn char_mask(cache: __m256i, needle: u8) -> u32 {
    let seach_mask = _mm256_set1_epi8(needle as i8);
    let eq_mask = _mm256_cmpeq_epi8(cache, seach_mask);
    _mm256_movemask_epi8(eq_mask) as u32
  }
}
