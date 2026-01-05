use std::{hint::unreachable_unchecked, slice};

use crate::{
  temperature_reading::{TemperatureReading, MAX_TEMP_READING_LEN},
  util::{unaligned_read_would_cross_page_boundary, unlikely, BitVector},
};

#[cfg(not(target_feature = "avx2"))]
use crate::scanner_cache::{read_next_from_buffer, BYTES_PER_BUFFER};
#[cfg(target_feature = "avx2")]
use crate::scanner_cache_x86::{read_next_from_buffer, BYTES_PER_BUFFER};

const MAX_STATION_NAME_LEN: usize = 50;
/// The amount of overlapping bytes between consecutive buffers in
/// multithreaded mode.
pub const BUFFER_OVERLAP: usize = (MAX_STATION_NAME_LEN
  + std::mem::size_of_val(&b';')
  + MAX_TEMP_READING_LEN
  + std::mem::size_of_val(&b'\n'))
.next_multiple_of(BYTES_PER_BUFFER);

pub(crate) const SCANNER_CACHE_SIZE: usize = BYTES_PER_BUFFER;

/// Scans for alternating semicolons and newlines.
pub struct Scanner<'a> {
  buffer: &'a [u8],
  semicolon_mask: u64,
  newline_mask: u64,

  /// The offset of the previously-read newline character + 1, e.g. the
  /// starting point of the expected next weather station name.
  cur_offset: u32,
}

impl<'a> Scanner<'a> {
  /// Constructs a Scanner over a buffer, which must be aligned to 32 bytes.
  pub fn from_start<'b: 'a>(buffer: &'b [u8]) -> Self {
    debug_assert!(buffer.len().is_multiple_of(BYTES_PER_BUFFER));
    let (semicolon_mask, newline_mask) = read_next_from_buffer(buffer);
    Self {
      buffer,
      semicolon_mask,
      newline_mask,
      cur_offset: 0,
    }
  }

  /// Finds the point we should start iterating from, assuming the first
  /// `BUFFER_OVERLAP` bytes are overlapping with the previous buffer. We
  /// choose to start iterating after the last newline character found in the
  /// overlap region, since this is naturally where the scanner iterating over
  /// the previous slice would stop.
  fn find_starting_point_in_overlap(buffer: &[u8]) -> (&[u8], u64, u64, u32) {
    let (mut semicolon_mask, mut newline_mask) = read_next_from_buffer(buffer);
    let mut buffer_offset = 0;
    #[allow(clippy::reversed_empty_ranges)]
    for offset in (BYTES_PER_BUFFER..BUFFER_OVERLAP).step_by(BYTES_PER_BUFFER) {
      let (next_semicolon_mask, next_newline_mask) = read_next_from_buffer(&buffer[offset..]);
      if next_newline_mask != 0 {
        buffer_offset = offset;
        semicolon_mask = next_semicolon_mask;
        newline_mask = next_newline_mask;
      }
    }
    let buffer = &buffer[buffer_offset..];
    debug_assert_ne!(newline_mask, 0);
    if newline_mask == 0 {
      unsafe { unreachable_unchecked() };
    }

    let cur_offset = newline_mask.ilog2();
    if cur_offset == BYTES_PER_BUFFER as u32 - 1 {
      let buffer = &buffer[BYTES_PER_BUFFER..];
      let (semicolon_mask, newline_mask) = read_next_from_buffer(buffer);
      (buffer, semicolon_mask, newline_mask, 0)
    } else {
      let remove_mask = !((2 << cur_offset) - 1);
      (
        buffer,
        semicolon_mask & remove_mask,
        newline_mask & remove_mask,
        cur_offset + 1,
      )
    }
  }

  /// Constructs a scanner that begins iterating at a point immediately
  /// proceeding a scanner iterating over the previous slice from the file,
  /// assuming the first `BUFFER_OVERLAP` bytes are overlapping with the
  /// previous slice.
  pub fn from_midpoint<'b: 'a>(buffer: &'b [u8]) -> Self {
    debug_assert!(buffer.len() >= BUFFER_OVERLAP);
    debug_assert!(buffer.len().is_multiple_of(BYTES_PER_BUFFER));
    let (buffer, semicolon_mask, newline_mask, cur_offset) =
      Self::find_starting_point_in_overlap(buffer);
    Self {
      buffer,
      semicolon_mask,
      newline_mask,
      cur_offset,
    }
  }

  /// Reads in the next `BYTES_PER_BUFFER` bytes from the buffer and updates
  /// the semicolon/newline bitmasks. This method assumes that we are not at
  /// the end of the file.
  fn read_next_assuming_available(&mut self) {
    debug_assert!(self.buffer.len() > BYTES_PER_BUFFER);
    self.buffer = unsafe { self.buffer.get_unchecked(BYTES_PER_BUFFER..) };
    let (semicolon_mask, newline_mask) = read_next_from_buffer(self.buffer);
    self.semicolon_mask = semicolon_mask;
    self.newline_mask = newline_mask;
  }

  /// In mutlithreading mode, the end of our buffer may not be the end of the
  /// entire file, so we can't assume there will be a newline following the
  /// last semicolon. Therefore, we must always check that we haven't reached
  /// the end of the buffer when reading in more data.
  #[must_use]
  #[cfg(feature = "multithreaded")]
  fn read_next_assuming_available_if_single_thread(&mut self) -> bool {
    self.read_next()
  }
  /// In single threaded mode, the end of the buffer is the end of the file, so
  /// we can assume there is a newline following every semicolon. If the
  /// current buffer has a semicolon near the end but no newline following, we
  /// know there must be at least one more buffer's worth of contents
  /// remaining. Otherwise the file format would be invalid.
  #[must_use]
  #[cfg(not(feature = "multithreaded"))]
  fn read_next_assuming_available_if_single_thread(&mut self) -> bool {
    self.read_next_assuming_available();
    true
  }

  /// Reads the next `BYTES_PER_BUFFER` bytes from the buffer, updating the
  /// internal state of `self` and returning `true` if there were more bytes to
  /// read, or returning `false` if EOF was reached.
  #[must_use]
  fn read_next(&mut self) -> bool {
    debug_assert!(!self.buffer.is_empty());
    if self.buffer.len() == BYTES_PER_BUFFER {
      return false;
    }
    self.read_next_assuming_available();
    true
  }

  fn offset_to_ptr(&self, offset: u32) -> *const u8 {
    debug_assert!(offset <= BYTES_PER_BUFFER as u32);
    unsafe { self.buffer.get_unchecked(offset as usize..) }.as_ptr()
  }

  /// Reads bytes from the buffer into the cache while no newline characters
  /// are in the cache, returning `true` if a newline character was eventually
  /// found. `false` indicates EOF was reached.
  #[must_use]
  fn read_until_next_semicolon(&mut self) -> bool {
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
      .saturating_sub(BYTES_PER_BUFFER)
      .div_ceil(BYTES_PER_BUFFER);
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

  /// Finds and returns a `str` spanning the name of the weather station on the
  /// next line to process. Returns `None` if EOF was reached.
  fn find_next_station_name(&mut self) -> Option<&'a str> {
    // Pointer to the start of the next station name.
    let station_start = self.offset_to_ptr(self.cur_offset);
    if !self.read_until_next_semicolon() {
      return None;
    }

    debug_assert!(
      self.semicolon_mask != 0,
      "Expected non-empty semicolon mask after refreshing buffers in iteration"
    );
    let semicolon_offset = self.semicolon_mask.pop_lsb();

    let station_end = self.offset_to_ptr(semicolon_offset);
    let station_name_slice = unsafe {
      slice::from_raw_parts::<'a>(
        station_start,
        station_end.byte_offset_from_unsigned(station_start),
      )
    };
    let station_name = unsafe { str::from_utf8_unchecked(station_name_slice) };

    self.cur_offset = semicolon_offset + 1;
    if semicolon_offset == BYTES_PER_BUFFER as u32 - 1 {
      if !self.read_next_assuming_available_if_single_thread() {
        return None;
      }
      self.cur_offset = 0;
    }

    debug_assert!(
      !station_name.contains('\n'),
      "Station name invalid: \"{station_name}\""
    );
    Some(station_name)
  }

  #[must_use]
  fn refresh_buffer_for_trailing_temp(&mut self) -> bool {
    if !self.read_next_assuming_available_if_single_thread() {
      return false;
    }

    debug_assert!(self.newline_mask != 0);
    let newline_offset = self.newline_mask.trailing_zeros();
    self.cur_offset = newline_offset + 1;
    debug_assert!(self.cur_offset < BYTES_PER_BUFFER as u32);
    true
  }

  fn parse_temp_from_copied_buffer(&mut self, start_offset: u32) -> Option<TemperatureReading> {
    debug_assert!(BYTES_PER_BUFFER >= std::mem::size_of::<u64>());
    const TMP_OFFSET: usize = BYTES_PER_BUFFER - std::mem::size_of::<u64>();
    debug_assert!(
      (TMP_OFFSET..BYTES_PER_BUFFER).contains(&(start_offset as usize)),
      "{TMP_OFFSET}..={BYTES_PER_BUFFER} does not contain {start_offset}"
    );

    let mut temp_storage = [0u64; 2];
    temp_storage[0] = unsafe { *(self.buffer.as_ptr().byte_add(TMP_OFFSET) as *const u64) };

    if self.newline_mask == 0 {
      if !self.refresh_buffer_for_trailing_temp() {
        return None;
      }
      temp_storage[1] = unsafe { *(self.buffer.as_ptr() as *const u64) };
    }

    Some(TemperatureReading::from_raw_ptr(unsafe {
      temp_storage
        .as_ptr()
        .byte_add(start_offset as usize - TMP_OFFSET) as *const u8
    }))
  }

  fn find_next_temp_reading(&mut self) -> Option<TemperatureReading> {
    let start_offset = self.cur_offset;
    let temp_start_ptr = self.offset_to_ptr(start_offset);
    let start_ptr = unsafe { self.buffer.get_unchecked(start_offset as usize..) }.as_ptr();

    // Slow path in case we are in danger of reading across a page boundary.
    let reading = if unlikely(unaligned_read_would_cross_page_boundary::<u64>(start_ptr)) {
      self.parse_temp_from_copied_buffer(start_offset)?
    } else {
      if self.newline_mask == 0 && !self.refresh_buffer_for_trailing_temp() {
        return None;
      }

      TemperatureReading::from_raw_ptr(temp_start_ptr)
    };

    self.cur_offset = self.newline_mask.pop_lsb() + 1;
    Some(reading)
  }
}

impl<'a> Iterator for Scanner<'a> {
  type Item = (&'a str, TemperatureReading);

  fn next(&mut self) -> Option<Self::Item> {
    let station_name = self.find_next_station_name()?;
    let temperature_reading = self.find_next_temp_reading()?;
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

    let mut scanner = Scanner::from_start(&buffer.buffer);
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

    let mut scanner = Scanner::from_start(&buffer.buffer);
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
        b'R', b'o', b'1', b';', b'2', b'.', b'3', b'\n', //
        b'R', b'o', b'1', b';', b'2', b'.', b'3', b'\n', //
        b'R', b'o', b'1', b';', b'2', b'.', b'3', b'\n', //
        b'R', b'o', b'1', b';', b'2', b'.', b'3', b'\n', //
        b'R', b'o', b'1', b';', b'2', b'.', b'3', b'\n', //
        b'R', b'o', b'1', b';', b'2', b'.', b'3', b'\n', //
        b'R', b'o', b'1', b';', b'2', b'.', b'3', b'\n', //
        b'R', b'o', b'1', b';', b'2', b'.', b'3', b'\n', //
        b'R', b'o', b'2', b';', b'3', b'.', b'4', b'\n', //
        0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
      ],
    };

    let mut scanner = Scanner::from_start(&buffer.buffer);
    for _ in 0..8 {
      expect_that!(
        scanner.next(),
        some((eq("Ro1"), eq(TemperatureReading::new(23))))
      );
    }
    expect_that!(
      scanner.next(),
      some((eq("Ro2"), eq(TemperatureReading::new(34))))
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

    let mut scanner = Scanner::from_start(&buffer.buffer);
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

    let mut scanner = Scanner::from_start(&buffer.buffer);
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

    let mut scanner = Scanner::from_start(&buffer.buffer);
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

    let mut scanner = Scanner::from_start(&buffer.buffer);
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

    let scanner = Scanner::from_start(input.padded_slice());
    let simple_scanner = simple_scanner_iter(input.padded_slice());
    expect_eq!(scanner.collect_vec(), simple_scanner.collect_vec());
  }

  #[gtest]
  #[ignore]
  fn test_against_large() {
    let input = random_input_file(17, 400_000, 10_000).unwrap();

    let scanner = Scanner::from_start(input.padded_slice());
    let simple_scanner = simple_scanner_iter(input.padded_slice());
    expect_eq!(scanner.collect_vec(), simple_scanner.collect_vec());
  }

  #[gtest]
  fn test_iter_from_midpoint_name_crosses_over() {
    let buffer = AlignedBuffer {
      buffer: *b"city1;3.4\ncity2;\
                 5.6\ncity3;7.8\nci\
                 ti4;9.0\ncity6;0.\
                 1\ncity7;2.3\ncity\
                 8;4.5\n\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    };

    let mut scanner = Scanner::from_midpoint(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((eq("city8"), eq(TemperatureReading::new(45))))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_from_midpoint_newline_at_end() {
    let buffer = AlignedBuffer {
      buffer: *b"city1;3.4\ncity2;\
                 5.6\ncity3;7.8\nci\
                 ti4;9.0\ncity6;0.\
                 1\nlong city;2.3\n\
                 city8;4.5\n\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    };

    let mut scanner = Scanner::from_midpoint(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((eq("city8"), eq(TemperatureReading::new(45))))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_from_midpoint_newline_at_start_of_next() {
    let buffer = AlignedBuffer {
      buffer: *b"city1;3.4\ncity2;\
                 5.6\ncity3;7.8\nci\
                 ti4;9.0\ncity6;0.\
                 1\nlong city1;2.3\
                 \ncity8;4.5\n\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    };

    let mut scanner = Scanner::from_midpoint(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((eq("long city1"), eq(TemperatureReading::new(23))))
    );
    expect_that!(
      scanner.next(),
      some((eq("city8"), eq(TemperatureReading::new(45))))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_from_midpoint_max_length_item() {
    let buffer = AlignedBuffer {
      buffer: *b"This is a city n\
                 ame which has th\
                 e most character\
                 s!;-10.2\nThis ci\
                 ty ain't so bad \
                 either;2.3\n\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    };

    let mut scanner = Scanner::from_midpoint(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((
        eq("This city ain't so bad either"),
        eq(TemperatureReading::new(23))
      ))
    );
    expect_that!(scanner.next(), none());
  }

  #[gtest]
  fn test_iter_from_midpoint_no_leading_semicolon() {
    let buffer = AlignedBuffer {
      buffer: *b"Kabinda;-17.5\nAb\
                 akaliki;16.8\nTro\
                 yes;31.9\nR\xc3\xabo Ca\
                 ribe;2.4\nUelzen;\
                 63.2\nMilton Keyn\
                 es;56.0\nZemrane;\
                 18.2\nImola;49.9\n\
                 Fulshear;23.9\nSa\
                 ndy Shores;15.0\n\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\
                 \0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    };

    let mut scanner = Scanner::from_midpoint(&buffer.buffer);
    expect_that!(
      scanner.next(),
      some((eq("Uelzen"), eq(TemperatureReading::new(632))))
    );
    expect_that!(
      scanner.next(),
      some((eq("Milton Keynes"), eq(TemperatureReading::new(560))))
    );
    expect_that!(
      scanner.next(),
      some((eq("Zemrane"), eq(TemperatureReading::new(182))))
    );
    expect_that!(
      scanner.next(),
      some((eq("Imola"), eq(TemperatureReading::new(499))))
    );
    expect_that!(
      scanner.next(),
      some((eq("Fulshear"), eq(TemperatureReading::new(239))))
    );
    expect_that!(
      scanner.next(),
      some((eq("Sandy Shores"), eq(TemperatureReading::new(150))))
    );
    expect_that!(scanner.next(), none());
  }
}
