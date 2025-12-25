use std::{cmp::Ordering, collections::HashMap, fmt::Display, fs::File, slice};

use itertools::Itertools;
use memmap2::{Advice, MmapOptions};

use crate::{error::BarseResult, scanner::Scanner, temperature_reading::TemperatureReading};

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
    TemperatureReading::new(avg as i16)
  }

  fn add_reading(&mut self, temp: TemperatureReading) {
    self.min = self.min.min(temp);
    self.max = self.max.max(temp);
    self.total += temp.reading() as i64;
    self.count += 1;
  }
}

impl Default for TemperatureSummary {
  fn default() -> Self {
    Self {
      min: TemperatureReading::new(i16::MAX),
      max: TemperatureReading::new(i16::MIN),
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

unsafe fn round_up_to_32b_boundary(buffer: &[u8]) -> &[u8] {
  unsafe { slice::from_raw_parts(buffer.as_ptr(), buffer.len().next_multiple_of(32)) }
}

pub fn temperature_reading_summaries(
  input_path: &str,
) -> BarseResult<impl Iterator<Item = WeatherStation>> {
  let file = File::open(input_path)?;
  let map = unsafe { MmapOptions::new().map(&file) }?;
  map.advise(Advice::Sequential)?;
  let map_buffer = unsafe { round_up_to_32b_boundary(&map) };

  Ok(
    Scanner::new(map_buffer)
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
