use std::{cmp::Ordering, fmt::Display, fs::File, slice};

use memmap2::{Advice, MmapOptions};

#[cfg(not(feature = "multithreaded"))]
use crate::build_table::build_temperature_reading_table_from_bytes;
#[cfg(feature = "multithreaded")]
use crate::build_table_mt::build_temperature_reading_table_from_bytes;

use crate::{
  error::BarseResult, scanner::SCANNER_CACHE_SIZE, temperature_summary::TemperatureSummary,
  util::HasIter,
};

unsafe fn round_up_to_cache_size_boundary(buffer: &[u8]) -> &[u8] {
  unsafe {
    slice::from_raw_parts(
      buffer.as_ptr(),
      buffer.len().next_multiple_of(SCANNER_CACHE_SIZE),
    )
  }
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

pub fn build_temperature_reading_table(
  input_path: &str,
) -> BarseResult<impl for<'a> HasIter<'a, Item = (&'a str, &'a TemperatureSummary)>> {
  let file = File::open(input_path)?;
  let map = unsafe { MmapOptions::new().map(&file) }?;
  map.advise(Advice::Sequential)?;

  let map_buffer = unsafe { round_up_to_cache_size_boundary(&map) };
  build_temperature_reading_table_from_bytes(map_buffer)
}
