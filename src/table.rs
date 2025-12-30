use std::{
  fmt::Debug,
  hash::{BuildHasher, Hasher},
  i16,
};

use memmap2::{MmapMut, MmapOptions};

use crate::{inline_string::InlineString, temperature_reading::TemperatureReading, util::likely};

#[derive(Debug, Clone, Copy)]
pub struct TemperatureSummary {
  min: TemperatureReading,
  max: TemperatureReading,
  total: i64,
  count: u32,
}

impl TemperatureSummary {
  fn initialize(&mut self) {
    self.min = TemperatureReading::new(i16::MAX);
    self.max = TemperatureReading::new(i16::MIN);
    debug_assert_eq!(self.total, 0);
    debug_assert_eq!(self.count, 0);
  }

  pub fn min(&self) -> TemperatureReading {
    self.min
  }

  pub fn max(&self) -> TemperatureReading {
    self.max
  }

  pub fn avg(&self) -> TemperatureReading {
    let rounding_offset = self.count as i64 / 2;
    let avg = (self.total + rounding_offset).div_euclid(self.count as i64);
    debug_assert!((i16::MIN as i64..=i16::MAX as i64).contains(&avg));
    TemperatureReading::new(avg as i16)
  }

  pub fn add_reading(&mut self, temp: TemperatureReading) {
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

#[derive(Default, Clone)]
struct Entry {
  key: InlineString,
  temp_summary: TemperatureSummary,
}

impl Entry {
  fn initialize(&mut self, station: &str) {
    self.key.initialize(station);
  }

  fn add_reading(&mut self, reading: TemperatureReading) {
    debug_assert!(!self.is_default());
    self.temp_summary.add_reading(reading);
  }

  fn matches_key_or_initialize(&mut self, station: &str) -> bool {
    if likely(self.key.eq_foreign_str(station)) {
      true
    } else if self.is_default() {
      self.initialize(station);
      true
    } else {
      false
    }
  }

  fn is_default(&self) -> bool {
    self.key.is_default()
  }

  fn to_iter_pair(&self) -> (&str, &TemperatureSummary) {
    (self.key.value_str(), &self.temp_summary)
  }
}

pub struct WeatherStationTable<const SIZE: usize, H> {
  buckets: MmapMut,
  hasher: H,
}

impl<const SIZE: usize, H> WeatherStationTable<SIZE, H> {
  pub fn iter(&self) -> impl Iterator<Item = (&str, &TemperatureSummary)> {
    WeatherStationIterator {
      table: self,
      index: 0,
    }
  }

  fn elements_ptr(&self) -> *const Entry {
    self.buckets.as_ptr() as *const Entry
  }

  fn mut_elements_ptr(&mut self) -> *mut Entry {
    self.buckets.as_mut_ptr() as *mut Entry
  }

  fn entry_at(&self, index: usize) -> &Entry {
    debug_assert!(index < SIZE);
    unsafe { &*self.elements_ptr().add(index) }
  }

  fn entry_at_mut(&mut self, index: usize) -> &mut Entry {
    debug_assert!(index < SIZE);
    unsafe { &mut *self.mut_elements_ptr().add(index) }
  }

  fn scan_for_entry(&mut self, station: &str, start_idx: usize) -> &mut Entry {
    let idx = (1..SIZE)
      .map(|i| (start_idx + i) % SIZE)
      .find(|&idx| self.entry_at_mut(idx).matches_key_or_initialize(station))
      .expect("No empty bucket found, table is full");
    self.entry_at_mut(idx)
  }
}

impl<const SIZE: usize, H: BuildHasher> WeatherStationTable<SIZE, H> {
  pub fn with_hasher(hasher: H) -> Self {
    let size = (SIZE * std::mem::size_of::<Entry>()).next_multiple_of(2 * 1024 * 1024);
    let buckets = MmapOptions::new().len(size).map_anon().unwrap();
    buckets.advise(memmap2::Advice::HugePage).unwrap();

    let mut s = Self { buckets, hasher };
    for i in 0..SIZE {
      s.entry_at_mut(i).temp_summary.initialize();
    }
    s
  }

  pub fn add_reading(&mut self, station: &str, reading: TemperatureReading) {
    self.find_entry(station).add_reading(reading);
  }

  fn station_hash(&self, station: &str) -> u64 {
    let mut hasher = self.hasher.build_hasher();
    hasher.write(station.as_bytes());
    hasher.finish()
  }

  fn station_index(&self, station: &str) -> usize {
    self.station_hash(station) as usize % SIZE
  }

  fn find_entry(&mut self, station: &str) -> &mut Entry {
    let idx = self.station_index(station);

    if likely(self.entry_at_mut(idx).matches_key_or_initialize(station)) {
      return self.entry_at_mut(idx);
    }

    // Otherwise we have to search for a bucket.
    self.scan_for_entry(station, idx)
  }
}

impl<const SIZE: usize, H: BuildHasher + Default> Default for WeatherStationTable<SIZE, H> {
  fn default() -> Self {
    Self::with_hasher(H::default())
  }
}

impl<const SIZE: usize, H> Debug for WeatherStationTable<SIZE, H> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "")
  }
}

struct WeatherStationIterator<'a, const SIZE: usize, H> {
  table: &'a WeatherStationTable<SIZE, H>,
  index: usize,
}

impl<'a, const SIZE: usize, H> Iterator for WeatherStationIterator<'a, SIZE, H> {
  type Item = (&'a str, &'a TemperatureSummary);

  fn next(&mut self) -> Option<Self::Item> {
    while self.index < SIZE {
      let entry = self.table.entry_at(self.index);
      self.index += 1;
      if !entry.is_default() {
        return Some(entry.to_iter_pair());
      }
    }
    None
  }
}

#[cfg(test)]
mod tests {
  use std::hash::RandomState;

  use googletest::prelude::*;
  use itertools::Itertools;

  use crate::{
    table::{TemperatureSummary, WeatherStationTable},
    temperature_reading::TemperatureReading,
  };

  #[gtest]
  fn test_insert() {
    let mut table = WeatherStationTable::<16, RandomState>::default();
    table.add_reading("station1", TemperatureReading::new(123));

    let mut iter = table.iter();
    expect_that!(
      iter.next(),
      some((
        eq("station1"),
        pat!(TemperatureSummary {
          min: &TemperatureReading::new(123),
          max: &TemperatureReading::new(123),
          total: &123,
          count: &1,
        })
      ))
    );
  }

  #[gtest]
  fn test_insert_two_stations() {
    let mut table = WeatherStationTable::<16, RandomState>::default();
    table.add_reading("station1", TemperatureReading::new(123));
    table.add_reading("station2", TemperatureReading::new(456));

    let elements = table.iter().collect_vec();
    expect_that!(
      elements,
      unordered_elements_are![
        (
          eq(&"station1"),
          derefs_to(pat!(TemperatureSummary {
            min: &TemperatureReading::new(123),
            max: &TemperatureReading::new(123),
            total: &123,
            count: &1,
          }))
        ),
        (
          eq(&"station2"),
          derefs_to(pat!(TemperatureSummary {
            min: &TemperatureReading::new(456),
            max: &TemperatureReading::new(456),
            total: &456,
            count: &1,
          }))
        )
      ]
    );
  }

  #[gtest]
  fn test_insert_station_twice() {
    let mut table = WeatherStationTable::<16, RandomState>::default();
    table.add_reading("station1", TemperatureReading::new(123));
    table.add_reading("station1", TemperatureReading::new(456));

    let elements = table.iter().collect_vec();
    expect_that!(
      elements,
      elements_are![(
        eq(&"station1"),
        derefs_to(pat!(TemperatureSummary {
          min: &TemperatureReading::new(123),
          max: &TemperatureReading::new(456),
          total: &579,
          count: &2,
        }))
      )]
    );
  }
}
