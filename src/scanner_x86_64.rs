use std::{
  arch::x86_64::{
    __m256i, _mm256_cmpeq_epi8, _mm256_load_si256, _mm256_movemask_epi8, _mm256_set1_epi8,
    _mm256_store_si256,
  },
  ptr::read_unaligned,
  slice,
};

use crate::temperature_reading::TemperatureReading;

pub struct ScannerX86<'a> {
  buffer: &'a [u8],
  cache: __m256i,
  semicolon_mask: u32,
  newline_mask: u32,

  /// The offset of the previously-read newline character + 1, e.g. the
  /// starting point of the expected next weather station name.
  cur_offset: u32,
}

impl<'a> ScannerX86<'a> {
  /// The count of bytes that fit in a __m256i
  const BYTES_PER_BUFFER: u32 = 32;

  /// Constructs a Scanner over a buffer, which must be aligned to 32 bytes.
  pub fn new<'b: 'a>(buffer: &'b [u8]) -> Self {
    debug_assert!(buffer.len().is_multiple_of(Self::BYTES_PER_BUFFER as usize));
    let (cache, semicolon_mask, newline_mask) = unsafe { Self::read_next_from_buffer(buffer) };
    Self {
      buffer,
      cache,
      semicolon_mask,
      newline_mask,
      cur_offset: 0,
    }
  }

  #[target_feature(enable = "avx2")]
  fn read_next_from_buffer(buffer: &[u8]) -> (__m256i, u32, u32) {
    let cache = unsafe { _mm256_load_si256(buffer.as_ptr() as *const __m256i) };
    let semicolon_mask = Self::char_mask(cache, b';');
    let newline_mask = Self::char_mask(cache, b'\n');
    (cache, semicolon_mask, newline_mask)
  }

  #[target_feature(enable = "avx2")]
  fn char_mask(cache: __m256i, needle: u8) -> u32 {
    let seach_mask = _mm256_set1_epi8(needle as i8);
    let eq_mask = _mm256_cmpeq_epi8(cache, seach_mask);
    _mm256_movemask_epi8(eq_mask) as u32
  }

  fn read_next_assuming_available(&mut self) {
    debug_assert!(self.buffer.len() > Self::BYTES_PER_BUFFER);
    self.buffer = &self.buffer[Self::BYTES_PER_BUFFER..];
    let (cache, semicolon_mask, newline_mask) = unsafe { Self::read_next_from_buffer(self.buffer) };
    self.cache = cache;
    self.semicolon_mask = semicolon_mask;
    self.newline_mask = newline_mask;
  }

  fn read_next(&mut self) -> bool {
    debug_assert!(!self.buffer.is_empty());
    if self.buffer.len() == Self::BYTES_PER_BUFFER {
      return false;
    }
    self.read_next_assuming_available();
    true
  }

  fn offset_to_ptr(&self, offset: u32) -> *const u8 {
    debug_assert!(offset <= Self::BYTES_PER_BUFFER);
    unsafe { self.buffer.get_unchecked(offset as usize..) }.as_ptr()
  }

  pub fn find_next_station_name(&mut self) -> Option<&'a str> {
    let station_start = self.offset_to_ptr(self.cur_offset);
    if self.semicolon_mask == 0 {
      if !self.read_next() {
        return None;
      }

      // This can only occur if the next station name spanned the entire buffer
      // read, 32 characters. Since the maximum station name is 50 characters,
      // we are guaranteed to find the end of the station in the next region.
      if self.semicolon_mask == 0 && !self.read_next() {
        return None;
      }
    }

    debug_assert!(
      self.semicolon_mask != 0,
      "Expected non-empty semicolon mask after refreshing buffers in iteration"
    );
    let semicolon_offset = self.semicolon_mask.trailing_zeros();
    self.semicolon_mask &= self.semicolon_mask - 1;

    let station_end = self.offset_to_ptr(semicolon_offset);
    let station_name_slice = unsafe {
      slice::from_raw_parts::<'a>(
        station_start,
        station_end.byte_offset_from_unsigned(station_start),
      )
    };
    let station_name = unsafe { str::from_utf8_unchecked(station_name_slice) };

    self.cur_offset = semicolon_offset + 1;
    if semicolon_offset == 31 {
      self.read_next_assuming_available();
      self.cur_offset = 0;
    }

    Some(station_name)
  }

  fn refresh_buffer_for_trailing_temp(&mut self) {
    self.read_next_assuming_available();

    debug_assert!(self.newline_mask != 0);
    let newline_offset = self.newline_mask.trailing_zeros();
    self.cur_offset = newline_offset + 1;
    debug_assert!(self.cur_offset < Self::BYTES_PER_BUFFER);
  }

  fn unaligned_u64_read_would_cross_page_boundary(start_ptr: *const u8) -> bool {
    const PAGE_SIZE: usize = 4096;
    (start_ptr as usize) % PAGE_SIZE > PAGE_SIZE - (u64::BITS as usize / 8)
  }

  // #[cold]
  fn parse_temp_from_copied_buffer(&mut self, start_offset: u32) -> TemperatureReading {
    #[repr(align(64))]
    struct TempStorage([u8; 64]);

    let mut temp_storage = TempStorage([0; 64]);
    unsafe { _mm256_store_si256(temp_storage.0.as_mut_ptr() as *mut __m256i, self.cache) };

    if self.newline_mask == 0 {
      self.refresh_buffer_for_trailing_temp();
      unsafe {
        _mm256_store_si256(
          temp_storage.0[Self::BYTES_PER_BUFFER..].as_mut_ptr() as *mut __m256i,
          self.cache,
        )
      };
    }

    let encoding = unsafe {
      read_unaligned(
        temp_storage
          .0
          .get_unchecked(start_offset as usize..)
          .as_ptr() as *const u64,
      )
    };

    TemperatureReading::from_encoding(encoding)
  }

  #[target_feature(enable = "avx2")]
  fn find_next_temp_reading_avx(&mut self) -> TemperatureReading {
    let start_offset = self.cur_offset;
    let temp_start_ptr = self.offset_to_ptr(start_offset);
    let start_ptr = unsafe { self.buffer.get_unchecked(start_offset as usize..) }.as_ptr();

    // Slow path in case we are in danger of reading across a page boundary.
    let reading = if Self::unaligned_u64_read_would_cross_page_boundary(start_ptr) {
      self.parse_temp_from_copied_buffer(start_offset)
    } else {
      if self.newline_mask == 0 {
        self.refresh_buffer_for_trailing_temp();
      }

      TemperatureReading::from_raw_ptr(temp_start_ptr)
    };

    self.cur_offset = self.newline_mask.trailing_zeros() + 1;
    self.newline_mask &= self.newline_mask - 1;

    reading
  }

  pub fn find_next_temp_reading(&mut self) -> TemperatureReading {
    unsafe { self.find_next_temp_reading_avx() }
  }
}
