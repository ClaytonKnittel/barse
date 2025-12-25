use std::{
  arch::x86_64::{
    __m256i, _mm256_cmpeq_epi8, _mm256_load_si256, _mm256_movemask_epi8, _mm256_set1_epi8,
    _mm256_store_si256,
  },
  hint::unreachable_unchecked,
  ptr::read_unaligned,
  slice,
  str::FromStr,
};

use crate::temperature_reading::TemperatureReading;

const MAX_WEATHER_STATION_LEN: usize = 50;

/// Scans for alternating semicolons and newlines.
struct Scanner<'a> {
  buffer: &'a [u8],
  cache: __m256i,
  semicolon_mask: u32,
  newline_mask: u32,

  /// The offset of the previously-read newline character + 1, e.g. the
  /// starting point of the expected next weather station name.
  cur_offset: u32,
}

impl<'a> Scanner<'a> {
  /// The count of bytes that fit in a __m256i
  const BYTES_PER_BUFFER: u32 = 32;

  /// Constructs a Scanner over a buffer, which must be aligned to 32 bytes.
  pub fn new<'b: 'a>(buffer: &'b [u8]) -> Self {
    debug_assert!(buffer.len().is_multiple_of(32));
    let (cache, semicolon_mask, newline_mask) = unsafe { Self::read_next_from_buffer(buffer) };
    println!("semicolon mask: {semicolon_mask:08x}");
    println!("newline mask:   {newline_mask:08x}");
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
    debug_assert!(self.buffer.len() > 32);
    self.buffer = &self.buffer[32..];
    let (cache, semicolon_mask, newline_mask) = unsafe { Self::read_next_from_buffer(self.buffer) };
    self.cache = cache;
    self.semicolon_mask = semicolon_mask;
    self.newline_mask = newline_mask;
  }

  fn read_next(&mut self) -> bool {
    debug_assert!(!self.buffer.is_empty());
    if self.buffer.len() == 32 {
      return false;
    }
    self.read_next_assuming_available();
    true
  }

  fn offset_to_ptr(&self, offset: u32) -> *const u8 {
    debug_assert!(offset <= Self::BYTES_PER_BUFFER);
    unsafe { self.buffer.get_unchecked(offset as usize..) }.as_ptr()
  }

  fn find_next_station_name(&mut self) -> Option<&'a str> {
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
    debug_assert!(self.cur_offset < 32);
  }

  fn unaligned_u64_read_would_cross_page_boundary(start_ptr: *const u8) -> bool {
    const PAGE_SIZE: usize = 4096;
    (start_ptr as usize) % PAGE_SIZE > PAGE_SIZE - (u64::BITS as usize / 8)
  }

  // #[cold]
  fn parse_temp_from_copied_buffer(&self, start_offset: u32) -> TemperatureReading {
    #[repr(align(64))]
    struct TempStorage([u8; 64]);

    let mut temp_storage = TempStorage([0; 64]);
    unsafe { _mm256_store_si256(temp_storage.0.as_mut_ptr() as *mut __m256i, self.cache) };
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
  fn find_next_temp_reading(&mut self) -> TemperatureReading {
    let start_offset = self.cur_offset;
    let temp_start_ptr = self.offset_to_ptr(start_offset);
    let start_ptr = unsafe { self.buffer.get_unchecked(start_offset as usize..) }.as_ptr();

    if self.newline_mask == 0 {
      self.refresh_buffer_for_trailing_temp();
    }

    self.cur_offset = self.newline_mask.trailing_zeros() + 1;
    self.newline_mask &= self.newline_mask - 1;

    // Slow path in case we are in danger of reading across a page boundary.
    if Self::unaligned_u64_read_would_cross_page_boundary(start_ptr) {
      return self.parse_temp_from_copied_buffer(start_offset);
    }

    TemperatureReading::from_raw_ptr(temp_start_ptr)
  }
}

impl<'a> Iterator for Scanner<'a> {
  type Item = (&'a str, TemperatureReading);

  fn next(&mut self) -> Option<Self::Item> {
    let station_name = self.find_next_station_name()?;
    let temperature_reading = unsafe { self.find_next_temp_reading() };
    Some((station_name, temperature_reading))
  }
}

#[cfg(test)]
mod tests {
  use googletest::{gtest, prelude::*};

  use crate::temperature_reading::TemperatureReading;

  use super::Scanner;

  #[repr(align(32))]
  struct AlignedBuffer<const N: usize> {
    buffer: [u8; N],
  }

  #[gtest]
  fn test_iter_single_element() {
    let buffer = AlignedBuffer {
      buffer: [
        b'G', b'a', b's', b's', b'e', b'l', b't', b'e', b'r', b'b', b'o', b'e', b'r', b'v', b'e',
        b'e', b'n', b's', b'c', b'h', b'e', b'm', b'o', b'n', b'd', b';', b'-', b'1', b'2', b'.',
        b'3', b'\n',
      ],
    };

    let mut scanner = Scanner::new(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((
        eq("Gasselterboerveenschemond"),
        eq(TemperatureReading::new(-123))
      ))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_two_rows() {
    let buffer = AlignedBuffer {
      buffer: [
        b'A', b'b', b';', b'2', b'0', b'.', b'8', b'\n', //
        b'C', b'd', b';', b'1', b'.', b'9', b'\n', //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      ],
    };

    let mut scanner = Scanner::new(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((eq("Ab"), eq(TemperatureReading::new(208))))
    );
    expect_that!(
      scanner.next(),
      some((eq("Cd"), eq(TemperatureReading::new(19))))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_two_spans() {
    let buffer = AlignedBuffer {
      buffer: [
        b'A', b'b', b'c', b'd', b'e', b'f', b'g', b';', b'2', b'0', b'.', b'8', b'\n', //
        b'H', b'i', b'j', b'k', b'l', b'm', b';', b'-', b'9', b'8', b'.', b'7', b'\n', //
        b'N', b'o', b'p', b'q', b'r', b's', b't', b'u', b';', b'1', b'.', b'2', b'\n', //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      ],
    };

    let mut scanner = Scanner::new(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((eq("Abcdefg"), eq(TemperatureReading::new(208))))
    );
    expect_that!(
      scanner.next(),
      some((eq("Hijklm"), eq(TemperatureReading::new(-987))))
    );
    expect_that!(
      scanner.next(),
      some((eq("Nopqrstu"), eq(TemperatureReading::new(12))))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_ends_on_boundary() {
    let buffer = AlignedBuffer {
      buffer: [
        b'A', b'b', b'c', b'd', b'e', b'f', b'g', b'h', //
        b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', //
        b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', //
        b'y', b'z', b';', b'2', b'3', b'.', b'4', b'\n', //
        b'N', b'e', b'w', b' ', b'B', b'u', b'f', b'f', //
        b'e', b'r', b';', b'3', b'.', b'4', b'\n', 0, //
        0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0,
      ],
    };

    let mut scanner = Scanner::new(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((
        eq("Abcdefghijklmnopqrstuvwxyz"),
        eq(TemperatureReading::new(234))
      ))
    );
    expect_that!(
      scanner.next(),
      some((eq("New Buffer"), eq(TemperatureReading::new(34))))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_end_first_of_next_boundary() {
    let buffer = AlignedBuffer {
      buffer: [
        b'A', b'b', b'c', b'd', b'e', b'f', b'g', b'h', //
        b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', //
        b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', //
        b'y', b'z', b';', b'-', b'2', b'3', b'.', b'4', //
        b'\n', b'N', b'e', b'w', b' ', b'B', b'u', b'f', //
        b'f', b'e', b'r', b';', b'3', b'.', b'4', b'\n', //
        0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0,
      ],
    };

    let mut scanner = Scanner::new(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((
        eq("Abcdefghijklmnopqrstuvwxyz"),
        eq(TemperatureReading::new(-234))
      ))
    );
    expect_that!(
      scanner.next(),
      some((eq("New Buffer"), eq(TemperatureReading::new(34))))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_temp_crosses_boundary() {
    let buffer = AlignedBuffer {
      buffer: [
        b'A', b'b', b'c', b'd', b'e', b'f', b'g', b'h', //
        b'i', b'j', b'k', b'l', b'm', b'n', b'o', b'p', //
        b'q', b'r', b's', b't', b'u', b'v', b'w', b'x', //
        b'y', b'z', b'1', b'2', b'3', b';', b'-', b'2', //
        b'3', b'.', b'4', b'\n', b'N', b'e', b'w', b' ', //
        b'B', b'u', b'f', b'f', b'e', b'r', b';', b'3', //
        b'.', b'4', b'\n', 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0,
      ],
    };

    let mut scanner = Scanner::new(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((
        eq("Abcdefghijklmnopqrstuvwxyz123"),
        eq(TemperatureReading::new(-234))
      ))
    );
    expect_that!(
      scanner.next(),
      some((eq("New Buffer"), eq(TemperatureReading::new(34))))
    );
    expect_that!(scanner.next(), none());
  }
}
