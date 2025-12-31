use std::arch::x86_64::{
  __m256i, _mm256_cmpeq_epi8, _mm256_load_si256, _mm256_movemask_epi8, _mm256_set1_epi8,
};

pub const BYTES_PER_BUFFER: usize = 64;

#[target_feature(enable = "avx2")]
fn char_mask(cache: __m256i, needle: u8) -> u32 {
  let seach_mask = _mm256_set1_epi8(needle as i8);
  let eq_mask = _mm256_cmpeq_epi8(cache, seach_mask);
  _mm256_movemask_epi8(eq_mask) as u32
}

#[target_feature(enable = "avx2")]
fn read_next_from_buffer_avx(buffer: &[u8]) -> (u64, u64) {
  let m256_ptr = buffer.as_ptr() as *const __m256i;
  let cache1 = unsafe { _mm256_load_si256(m256_ptr) };
  let cache2 = unsafe { _mm256_load_si256(m256_ptr.add(1)) };

  let semicolon_mask = char_mask(cache1, b';') as u64 + ((char_mask(cache2, b';') as u64) << 32);
  let newline_mask = char_mask(cache1, b'\n') as u64 + ((char_mask(cache2, b'\n') as u64) << 32);
  (semicolon_mask, newline_mask)
}

pub fn read_next_from_buffer(buffer: &[u8]) -> (u64, u64) {
  unsafe { read_next_from_buffer_avx(buffer) }
}
