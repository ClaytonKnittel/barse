use crate::{scanner_x86_64::ScannerX86, temperature_reading::TemperatureReading};

/// Scans for alternating semicolons and newlines.
pub struct Scanner<'a> {
  scanner: ScannerX86<'a>,
}

impl<'a> Scanner<'a> {
  /// Constructs a Scanner over a buffer.
  pub fn new<'b: 'a>(buffer: &'b [u8]) -> Self {
    Self {
      scanner: ScannerX86::new(buffer),
    }
  }
}

impl<'a> Iterator for Scanner<'a> {
  type Item = (&'a str, TemperatureReading);

  fn next(&mut self) -> Option<Self::Item> {
    let station_name = self.scanner.find_next_station_name()?;
    let temperature_reading = self.scanner.find_next_temp_reading();
    Some((station_name, temperature_reading))
  }
}

#[cfg(test)]
mod tests {

  use std::{
    alloc::{alloc, dealloc, Layout},
    slice,
  };

  use brc::build_input::{get_weather_stations, output_lines};
  use googletest::{gtest, prelude::*};
  use itertools::Itertools;
  use rand::{rngs::StdRng, SeedableRng};

  use crate::{error::BarseResult, temperature_reading::TemperatureReading};

  use super::Scanner;

  const ALIGNMENT: usize = 32;

  #[repr(align(32))]
  struct AlignedBuffer<const N: usize> {
    buffer: [u8; N],
  }

  struct AlignedInput {
    bytes: *mut u8,
    len: usize,
  }
  impl AlignedInput {
    fn new(src: &str) -> Self {
      let len = src.len().next_multiple_of(ALIGNMENT);
      let layout = Layout::from_size_align(len, ALIGNMENT).unwrap();
      let bytes = unsafe { alloc(layout) };
      unsafe {
        libc::memset(bytes as *mut libc::c_void, 0, len);
        bytes.copy_from(src.as_bytes().as_ptr(), src.len());
      }
      Self { bytes, len }
    }

    fn slice(&self) -> &[u8] {
      unsafe { slice::from_raw_parts(self.bytes, self.len) }
    }
  }
  impl Drop for AlignedInput {
    fn drop(&mut self) {
      let layout = Layout::from_size_align(self.len, ALIGNMENT).unwrap();
      unsafe {
        dealloc(self.bytes, layout);
      }
    }
  }

  fn random_input_file(seed: u64, records: u64, unique_stations: u32) -> BarseResult<AlignedInput> {
    const WEATHER_STATIONS_PATH: &str = "data/weather_stations.csv";

    let mut rng = StdRng::seed_from_u64(seed);
    let stations = get_weather_stations(WEATHER_STATIONS_PATH).unwrap();

    Ok(AlignedInput::new(
      &output_lines(&stations, records, unique_stations, &mut rng)?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .join(""),
    ))
  }

  fn simple_scanner_iter(buffer: &[u8]) -> impl Iterator<Item = (&str, TemperatureReading)> {
    str::from_utf8(buffer)
      .unwrap()
      .split('\n')
      .filter(|line| !line.is_empty() && !line.starts_with(0 as char))
      .map(|line| {
        let (station, temp) = line.split_once(';').unwrap();
        let temp = (temp.parse::<f32>().unwrap() * 10.).round() as i16;
        (station, TemperatureReading::new(temp))
      })
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

    let scanner = Scanner::new(input.slice());
    let simple_scanner = simple_scanner_iter(input.slice());
    expect_eq!(scanner.collect_vec(), simple_scanner.collect_vec());
  }

  #[gtest]
  #[ignore]
  fn test_against_large() {
    let input = random_input_file(17, 400_000, 10_000).unwrap();

    let scanner = Scanner::new(input.slice());
    let simple_scanner = simple_scanner_iter(input.slice());
    expect_eq!(scanner.collect_vec(), simple_scanner.collect_vec());
  }
}
