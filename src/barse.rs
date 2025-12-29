use std::{cmp::Ordering, fmt::Display, fs::File, slice};

use memmap2::{Advice, MmapOptions};

use crate::{
  error::BarseResult,
  scanner::Scanner,
  table::{TemperatureSummary, WeatherStationTable},
};

const TABLE_SIZE: usize = 65536;

unsafe fn round_up_to_32b_boundary(buffer: &[u8]) -> &[u8] {
  unsafe { slice::from_raw_parts(buffer.as_ptr(), buffer.len().next_multiple_of(32)) }
}

pub struct WeatherStation<'a> {
  name: &'a str,
  summary: TemperatureSummary,
}

impl<'a> WeatherStation<'a> {
  pub fn new(name: &'a str, summary: TemperatureSummary) -> Self {
    Self { name, summary }
  }
}

impl<'a> PartialEq for WeatherStation<'a> {
  fn eq(&self, other: &Self) -> bool {
    self.name.eq(other.name)
  }
}

impl<'a> Eq for WeatherStation<'a> {}

impl<'a> PartialOrd for WeatherStation<'a> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<'a> Ord for WeatherStation<'a> {
  fn cmp(&self, other: &Self) -> Ordering {
    self.name.cmp(other.name)
  }
}

impl<'a> Display for WeatherStation<'a> {
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

pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
  let table = WeatherStationTable::new();

  Scanner::<TABLE_SIZE>::new(input, table.bucket_ptr_for_prefetch()).try_fold(
    table,
    |mut map, (station, station_hash, temp)| -> BarseResult<_> {
      map.add_reading(station, station_hash, temp);
      Ok(map)
    },
  )
}

pub fn build_temperature_reading_table(
  input_path: &str,
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
  let file = File::open(input_path)?;
  let map = unsafe { MmapOptions::new().map(&file) }?;
  map.advise(Advice::Sequential)?;

  let map_buffer = unsafe { round_up_to_32b_boundary(&map) };
  build_temperature_reading_table_from_bytes(map_buffer)
}
