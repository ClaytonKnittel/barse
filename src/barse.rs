use std::{cmp::Ordering, fmt::Display, fs::File, slice};

use memmap2::{Advice, MmapOptions};

use crate::{
  error::BarseResult,
  scanner::{Scanner, SCANNER_CACHE_SIZE},
  str_hash::TABLE_SIZE,
  table::{TemperatureSummary, WeatherStationTable},
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

#[cfg(feature = "multithreaded")]
pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
  use crate::error::BarseError;
  use itertools::Itertools;
  use std::sync::Arc;

  let thread_count = std::thread::available_parallelism()
    .map(|nonzero| nonzero.get())
    .unwrap_or(1);

  let slicer = Arc::new(unsafe { crate::slicer::Slicer::new(input) });

  let threads = (0..thread_count)
    .map(|_| {
      let slicer = slicer.clone();
      std::thread::spawn(move || {
        while let Some(slice) = slicer.next_slice() {
          for (station, temp) in slice {
            //
          }
        }
      })
    })
    .collect_vec();

  for thread in threads {
    thread
      .join()
      .map_err(|err| BarseError::new(format!("Failed to join thread: {err:?}")))?;
  }

  todo!();
}

#[cfg(not(feature = "multithreaded"))]
pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
  Ok(
    Scanner::from_start(input).fold(WeatherStationTable::new()?, |mut map, (station, temp)| {
      map.add_reading(station, temp);
      map
    }),
  )
}

pub fn build_temperature_reading_table(
  input_path: &str,
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
  let file = File::open(input_path)?;
  let map = unsafe { MmapOptions::new().map(&file) }?;
  map.advise(Advice::Sequential)?;

  let map_buffer = unsafe { round_up_to_cache_size_boundary(&map) };
  build_temperature_reading_table_from_bytes(map_buffer)
}
