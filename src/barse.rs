use std::{cmp::Ordering, collections::HashMap, ffi::c_void, fmt::Display, fs::File, str::FromStr};

use itertools::Itertools;
use memmap2::{Advice, MmapOptions};

use crate::error::{BarseError, BarseResult};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TemperatureReading {
  reading: i16,
}

impl FromStr for TemperatureReading {
  type Err = BarseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    debug_assert!((3..=5).contains(&s.len()));
    debug_assert_eq!(s.as_bytes()[s.len() - 2], b'.');
    let tens: i16 = unsafe { s[..s.len() - 2].parse().unwrap_unchecked() };
    let mut ones = (s.as_bytes()[s.len() - 1] - b'0') as i16;
    if s.as_bytes()[0] == b'-' {
      ones = -ones;
    }
    Ok(Self {
      reading: tens * 10 + ones,
    })
  }
}

impl Display for TemperatureReading {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let neg = if self.reading < 0 { "-" } else { "" };
    let tens = self.reading.abs() / 10;
    let ones = self.reading.abs() % 10;
    write!(f, "{neg}{tens}.{ones}")
  }
}

struct TemperatureSummary {
  min: TemperatureReading,
  max: TemperatureReading,
  total: i64,
  count: u32,
}

impl TemperatureSummary {
  fn min(&self) -> TemperatureReading {
    self.min
  }

  fn max(&self) -> TemperatureReading {
    self.max
  }

  fn avg(&self) -> TemperatureReading {
    let rounding_offset = self.count as i64 / 2;
    let avg = (self.total + rounding_offset).div_euclid(self.count as i64);
    debug_assert!((i16::MIN as i64..=i16::MAX as i64).contains(&avg));
    TemperatureReading {
      reading: avg as i16,
    }
  }

  fn add_reading(&mut self, temp: TemperatureReading) {
    self.min = self.min.min(temp);
    self.max = self.max.max(temp);
    self.total += temp.reading as i64;
    self.count += 1;
  }
}

impl Default for TemperatureSummary {
  fn default() -> Self {
    Self {
      min: TemperatureReading { reading: i16::MAX },
      max: TemperatureReading { reading: i16::MIN },
      total: 0,
      count: 0,
    }
  }
}

pub struct WeatherStation {
  name: String,
  summary: TemperatureSummary,
}

impl PartialEq for WeatherStation {
  fn eq(&self, other: &Self) -> bool {
    self.name.eq(&other.name)
  }
}

impl Eq for WeatherStation {}

impl PartialOrd for WeatherStation {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for WeatherStation {
  fn cmp(&self, other: &Self) -> Ordering {
    self.name.cmp(&other.name)
  }
}

impl Display for WeatherStation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}={}/{}/{}",
      self.name,
      self.summary.min(),
      self.summary.avg(),
      self.summary.max()
    )
  }
}

fn char_offset(buffer: *const u8, needle: u8, len: usize) -> usize {
  let semicolon = unsafe { libc::memchr(buffer as *const c_void, needle as i32, len) } as *const u8;
  unsafe { semicolon.offset_from(buffer) as usize }
}

fn parse_lines_from_buffer(mut buffer: &[u8]) -> impl Iterator<Item = (&str, TemperatureReading)> {
  const MAX_WEATHER_STATION_LEN: usize = 50;

  std::iter::from_fn(move || {
    if buffer.is_empty() {
      return None;
    }

    let semicolon_offset = char_offset(buffer.as_ptr(), b';', MAX_WEATHER_STATION_LEN + 1);
    let newline_offset =
      semicolon_offset + 1 + char_offset(buffer[semicolon_offset + 1..].as_ptr(), b'\n', 6);

    let station = unsafe { str::from_utf8_unchecked(&buffer[..semicolon_offset]) };
    let temp = unsafe {
      str::from_utf8_unchecked(&buffer[semicolon_offset + 1..newline_offset])
        .parse()
        .unwrap_unchecked()
    };

    buffer = &buffer[newline_offset + 1..];

    Some((station, temp))
  })
}

pub fn temperature_reading_summaries(
  input_path: &str,
) -> BarseResult<impl Iterator<Item = WeatherStation>> {
  let file = File::open(input_path)?;
  let map = unsafe { MmapOptions::new().map(&file) }?;
  map.advise(Advice::Sequential)?;

  Ok(
    parse_lines_from_buffer(&map)
      .try_fold(
        HashMap::<String, TemperatureSummary>::new(),
        |mut map, (station, temp)| -> BarseResult<_> {
          map.entry(station.to_owned()).or_default().add_reading(temp);
          Ok(map)
        },
      )?
      .into_iter()
      .map(|(station, summary)| WeatherStation {
        name: station,
        summary,
      })
      .sorted_unstable(),
  )
}
