use std::{ptr::read_unaligned, slice};

use crate::{
  temperature_reading::TemperatureReading,
  util::{unaligned_read_would_cross_page_boundary, unlikely},
};

#[cfg(not(target_feature = "avx2"))]
use crate::scanner_cache::Cache;
#[cfg(target_feature = "avx2")]
use crate::scanner_cache_x86::Cache;

const MAX_STATION_NAME_LEN: usize = 50;

/// Scans for alternating semicolons and newlines.
pub struct Scanner<'a> {
  buffer: &'a [u8],
  cache: Cache,
  semicolon_mask: u64,
  newline_mask: u64,

  /// The offset of the previously-read newline character + 1, e.g. the
  /// starting point of the expected next weather station name.
  cur_offset: u32,
}

impl<'a> Scanner<'a> {
  /// Constructs a Scanner over a buffer, which must be aligned to 32 bytes.
  pub fn new<'b: 'a>(buffer: &'b [u8]) -> Self {
    debug_assert!(buffer.len().is_multiple_of(Cache::BYTES_PER_BUFFER));
    let (cache, semicolon_mask, newline_mask) = Cache::read_next_from_buffer(buffer);
    Self {
      buffer,
      cache,
      semicolon_mask,
      newline_mask,
      cur_offset: 0,
    }
  }

  fn read_next_assuming_available(&mut self) {
    debug_assert!(self.buffer.len() > Cache::BYTES_PER_BUFFER);
    self.buffer = unsafe { self.buffer.get_unchecked(Cache::BYTES_PER_BUFFER..) };
    let (cache, semicolon_mask, newline_mask) = Cache::read_next_from_buffer(self.buffer);
    self.cache = cache;
    self.semicolon_mask = semicolon_mask;
    self.newline_mask = newline_mask;
  }

  fn read_next(&mut self) -> bool {
    debug_assert!(!self.buffer.is_empty());
    if self.buffer.len() == Cache::BYTES_PER_BUFFER {
      return false;
    }
    self.read_next_assuming_available();
    true
  }

  fn offset_to_ptr(&self, offset: u32) -> *const u8 {
    debug_assert!(offset <= Cache::BYTES_PER_BUFFER as u32);
    unsafe { self.buffer.get_unchecked(offset as usize..) }.as_ptr()
  }

  fn find_next_semicolon(&mut self) -> bool {
    if self.semicolon_mask != 0 {
      return true;
    } else if !self.read_next() {
      return false;
    }

    // The next semicolon must be found within the next MAX_STATION_NAME_LEN +
    // 1 bytes. In the worst case, the previous newline was the last character
    // of the previous buffer, and the read_next call we just performed read
    // the first `Cache::BYTES_PER_BUFFER` bytes of the next station name.
    // This means we may not find the next semicolon until
    // `MAX_STATION_NAME_LEN + 1 - Cache::bytes_per_buffer` more bytes have
    // been read.
    const MAX_ITERS: usize = (MAX_STATION_NAME_LEN + 1)
      .saturating_sub(Cache::BYTES_PER_BUFFER)
      .div_ceil(Cache::BYTES_PER_BUFFER);
    #[allow(clippy::reversed_empty_ranges)]
    for _ in 0..MAX_ITERS {
      if self.semicolon_mask != 0 {
        return true;
      } else if !self.read_next() {
        return false;
      }
    }
    true
  }

  fn find_next_station_name(&mut self) -> Option<&'a str> {
    let station_start = self.offset_to_ptr(self.cur_offset);
    if !self.find_next_semicolon() {
      return None;
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
    if semicolon_offset == Cache::BYTES_PER_BUFFER as u32 - 1 {
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
    debug_assert!(self.cur_offset < Cache::BYTES_PER_BUFFER as u32);
  }

  fn parse_temp_from_copied_buffer(&mut self, start_offset: u32) -> TemperatureReading {
    #[repr(align(64))]
    struct TempStorage([u8; 128]);

    let mut temp_storage = TempStorage([0; 128]);
    self.cache.aligned_store(temp_storage.0.as_mut_ptr());

    if self.newline_mask == 0 {
      self.refresh_buffer_for_trailing_temp();
      self
        .cache
        .aligned_store(temp_storage.0[Cache::BYTES_PER_BUFFER..].as_mut_ptr());
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

  fn find_next_temp_reading(&mut self) -> TemperatureReading {
    let start_offset = self.cur_offset;
    let temp_start_ptr = self.offset_to_ptr(start_offset);
    let start_ptr = unsafe { self.buffer.get_unchecked(start_offset as usize..) }.as_ptr();

    // Slow path in case we are in danger of reading across a page boundary.
    let reading = if unlikely(unaligned_read_would_cross_page_boundary::<u64>(start_ptr)) {
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
}

impl<'a> Iterator for Scanner<'a> {
  type Item = (&'a str, TemperatureReading);

  fn next(&mut self) -> Option<Self::Item> {
    let station_name = self.find_next_station_name()?;
    let temperature_reading = self.find_next_temp_reading();
    Some((station_name, temperature_reading))
  }
}

#[cfg(test)]
mod tests {
  use googletest::{gtest, prelude::*};
  use itertools::Itertools;

  use crate::{
    temperature_reading::TemperatureReading,
    test_util::{random_input_file, simple_scanner_iter, AlignedBuffer},
  };

  use super::Scanner;

  #[gtest]
  fn test_iter_single_element() {
    let buffer = AlignedBuffer {
      buffer: [
        b'G', b'a', b's', b's', b'e', b'l', b't', b'e', //
        b'r', b'b', b'o', b'e', b'r', b'v', b'e', b'e', //
        b'n', b's', b'c', b'h', b'e', b'm', b'o', b'n', //
        b'd', b';', b'-', b'1', b'2', b'.', b'3', b'\n', //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
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
        b'C', b'd', b';', b'1', b'.', b'9', b'\n', 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
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

  #[gtest]
  fn test_iter_many_places() {
    let buffer = AlignedBuffer {
      buffer: [
        b'P', b'1', b';', b'1', b'.', b'2', b'\n', b'P', //
        b'2', b';', b'3', b'.', b'4', b'\n', b'P', b'3', //
        b';', b'5', b'.', b'6', b'\n', b'P', b'4', b';', //
        b'7', b'.', b'8', b'\n', b'P', b'5', b';', b'9', //
        b'.', b'0', b'\n', 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, //
      ],
    };

    let mut scanner = Scanner::new(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((eq("P1"), eq(TemperatureReading::new(12))))
    );
    expect_that!(
      scanner.next(),
      some((eq("P2"), eq(TemperatureReading::new(34))))
    );
    expect_that!(
      scanner.next(),
      some((eq("P3"), eq(TemperatureReading::new(56))))
    );
    expect_that!(
      scanner.next(),
      some((eq("P4"), eq(TemperatureReading::new(78))))
    );
    expect_that!(
      scanner.next(),
      some((eq("P5"), eq(TemperatureReading::new(90))))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_against_small() {
    let input = random_input_file(13, 10_000, 1_000).unwrap();

    let scanner = Scanner::new(input.padded_slice());
    let simple_scanner = simple_scanner_iter(input.padded_slice());
    expect_eq!(scanner.collect_vec(), simple_scanner.collect_vec());
  }

  #[gtest]
  #[ignore]
  fn test_against_large() {
    let input = random_input_file(17, 400_000, 10_000).unwrap();

    let scanner = Scanner::new(input.padded_slice());
    let simple_scanner = simple_scanner_iter(input.padded_slice());
    expect_eq!(scanner.collect_vec(), simple_scanner.collect_vec());
  }
}
