use std::{
  arch::x86_64::{
    __m256i, _mm256_blendv_epi8, _mm256_castsi256_si128, _mm256_cmpeq_epi8, _mm256_load_si256,
    _mm256_movemask_epi8, _mm256_permute2f128_si256, _mm256_permute2x128_si256, _mm256_set1_epi8,
    _mm256_store_si256, _mm_cvtsi128_si64,
  },
  hint::unreachable_unchecked,
  ptr::read_unaligned,
  slice,
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
    let (buffer, cache, semicolon_mask, newline_mask) =
      unsafe { Self::read_next_from_buffer(buffer) };
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
  fn read_next_from_buffer(buffer: &[u8]) -> (&[u8], __m256i, u32, u32) {
    let cache = unsafe { _mm256_load_si256(buffer.as_ptr() as *const __m256i) };
    let semicolon_mask = Self::char_mask(cache, b';');
    let newline_mask = Self::char_mask(cache, b'\n');
    (&buffer[32..], cache, semicolon_mask, newline_mask)
  }

  #[target_feature(enable = "avx2")]
  fn char_mask(cache: __m256i, needle: u8) -> u32 {
    let seach_mask = _mm256_set1_epi8(needle as i8);
    let eq_mask = _mm256_cmpeq_epi8(cache, seach_mask);
    _mm256_movemask_epi8(eq_mask) as u32
  }

  fn read_next(&mut self) -> bool {
    if self.buffer.is_empty() {
      return false;
    }
    let (buffer, cache, semicolon_mask, newline_mask) =
      unsafe { Self::read_next_from_buffer(self.buffer) };
    self.buffer = buffer;
    self.cache = cache;
    self.semicolon_mask = semicolon_mask;
    self.newline_mask = newline_mask;
    true
  }

  fn offset_to_ptr(&self, offset: u32) -> *const u8 {
    debug_assert!(offset < Self::BYTES_PER_BUFFER);
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
        station_end.byte_offset_from(station_start) as usize,
      )
    };
    let station_name = unsafe { str::from_utf8_unchecked(station_name_slice) };

    self.cur_offset = semicolon_offset;
    if semicolon_offset == 32 {
      debug_assert!(!self.buffer.is_empty());
      if self.buffer.is_empty() {
        unsafe { unreachable_unchecked() };
      }

      let res = self.read_next();
      debug_assert!(res);
      self.cur_offset = 0;
    }

    Some(station_name)
  }

  #[target_feature(enable = "avx2")]
  fn find_next_temp_reading(&mut self) -> TemperatureReading {
    if self.newline_mask == 0 {
      // Slow path
      todo!();
    } else {
      // Fast path
      #[repr(align(32))]
      struct TempStorage([u8; 37]);

      let mut temp_storage = TempStorage([0; 37]);
      unsafe { _mm256_store_si256(temp_storage.0.as_mut_ptr() as *mut __m256i, self.cache) };
      let encoding = unsafe {
        read_unaligned(temp_storage.0[self.cur_offset as usize..].as_ptr() as *const u64)
      };

      self.cur_offset = self.newline_mask.trailing_zeros();
      self.newline_mask &= self.newline_mask - 1;

      TemperatureReading::from_encoding(encoding)
    }
  }
}

impl<'a> Iterator for Scanner<'a> {
  type Item = (&'a str, TemperatureReading);

  fn next(&mut self) -> Option<Self::Item> {
    let station_name = self.find_next_station_name()?;

    Some((station_name, TemperatureReading::new(0)))
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
  fn test_iter() {
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
}
