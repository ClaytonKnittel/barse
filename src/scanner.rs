use std::arch::x86_64::{
  __m256i, _mm256_bsrli_epi128, _mm256_cmpeq_epi8_mask, _mm256_load_si256, _mm256_set1_epi8,
};

use crate::temperature_reading::TemperatureReading;

const MAX_WEATHER_STATION_LEN: usize = 50;

/// Scans for alternating semicolons and newlines.
struct Scanner<'a> {
  buffer: &'a [u8],
  cache: __m256i,
  semicolon_mask: u32,
  newline_mask: u32,
  cur_offset: u32,
}

impl<'a> Scanner<'a> {
  /// Constructs a Scanner over a buffer, which must be aligned to 32 bytes.
  pub fn new<'b: 'a>(buffer: &'b [u8]) -> Self {
    debug_assert!(buffer.len().is_multiple_of(32));
    let (buffer, cache, semicolon_mask, newline_mask) =
      unsafe { Self::read_next_from_buffer(buffer) };
    Self {
      buffer,
      cache,
      semicolon_mask,
      newline_mask,
      cur_offset: 0,
    }
  }

  #[target_feature(enable = "avx512bw,avx512vl")]
  fn read_next_from_buffer(buffer: &[u8]) -> (&[u8], __m256i, u32, u32) {
    let cache = unsafe { _mm256_load_si256(buffer.as_ptr() as *const __m256i) };
    let semicolon_mask = Self::char_mask(cache, b';');
    let newline_mask = Self::char_mask(cache, b'\n');
    (&buffer[32..], cache, semicolon_mask, newline_mask)
  }

  #[target_feature(enable = "avx512bw,avx512vl")]
  fn char_mask(cache: __m256i, needle: u8) -> u32 {
    let seach_mask = _mm256_set1_epi8(needle as i8);
    _mm256_cmpeq_epi8_mask(cache, seach_mask)
  }
}

impl<'a> Iterator for Scanner<'a> {
  type Item = (&'a str, TemperatureReading);

  fn next(&mut self) -> Option<Self::Item> {
    None
  }
}
