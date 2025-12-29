use std::fmt::Debug;

use crate::{
  inline_string::InlineString, str_hash::station_hash, temperature_reading::TemperatureReading,
  util::likely,
};

#[derive(Debug, Clone, Copy)]
pub struct TemperatureSummary {
  min: TemperatureReading,
  max: TemperatureReading,
  total: i64,
  count: u32,
}

impl TemperatureSummary {
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

pub struct WeatherStationTable<const SIZE: usize> {
  buckets: Box<[Entry]>,
}

impl<const SIZE: usize> WeatherStationTable<SIZE> {
  pub fn new() -> Self {
    Self {
      buckets: vec![Entry::default(); SIZE].into_boxed_slice(),
    }
  }

  pub fn bucket_ptr_for_prefetch(&self) -> *const u8 {
    self.buckets.as_ptr() as *const u8
  }

  pub fn prefetch_bucket(buckets_ptr: *const u8, hash: u64) {
    let station_index = hash as usize % SIZE;
    unsafe {
      let bucket_ptr = (buckets_ptr as *const Entry).add(station_index);
      std::arch::x86_64::_mm_prefetch::<{ std::arch::x86_64::_MM_HINT_T0 }>(
        bucket_ptr as *const i8,
      );
    }
  }

  pub fn iter(&self) -> impl Iterator<Item = (&str, &TemperatureSummary)> {
    WeatherStationIterator {
      table: self,
      index: 0,
    }
  }

  fn entry_at(&self, index: usize) -> &Entry {
    unsafe { self.buckets.get_unchecked(index) }
  }

  fn entry_at_mut(&mut self, index: usize) -> &mut Entry {
    unsafe { self.buckets.get_unchecked_mut(index) }
  }

  fn scan_for_entry(&mut self, station: &str, start_idx: usize) -> &mut Entry {
    let idx = (1..SIZE)
      .map(|i| (start_idx + i) % SIZE)
      .find(|&idx| self.entry_at_mut(idx).matches_key_or_initialize(station))
      .expect("No empty bucket found, table is full");
    self.entry_at_mut(idx)
  }

  pub fn add_reading(&mut self, station: &str, hash: u64, reading: TemperatureReading) {
    self.find_entry(station, hash).add_reading(reading);
  }

  #[cfg(test)]
  fn add_reading_for_tests(&mut self, station: &str, reading: TemperatureReading) {
    self.add_reading(station, station_hash(station), reading);
  }

  fn station_index(&self, station: &str, hash: u64) -> usize {
    debug_assert_eq!(hash, station_hash(station));
    hash as usize % SIZE
  }

  fn find_entry(&mut self, station: &str, hash: u64) -> &mut Entry {
    let idx = self.station_index(station, hash);

    if likely(self.entry_at_mut(idx).matches_key_or_initialize(station)) {
      return self.entry_at_mut(idx);
    }

    // Otherwise we have to search for a bucket.
    self.scan_for_entry(station, idx)
  }
}

impl<const SIZE: usize> Default for WeatherStationTable<SIZE> {
  fn default() -> Self {
    Self::new()
  }
}

impl<const SIZE: usize> Debug for WeatherStationTable<SIZE> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "")
  }
}

struct WeatherStationIterator<'a, const SIZE: usize> {
  table: &'a WeatherStationTable<SIZE>,
  index: usize,
}

impl<'a, const SIZE: usize> Iterator for WeatherStationIterator<'a, SIZE> {
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
  use googletest::prelude::*;
  use itertools::Itertools;

  use crate::{
    table::{TemperatureSummary, WeatherStationTable},
    temperature_reading::TemperatureReading,
  };

  #[gtest]
  fn test_insert() {
    let mut table = WeatherStationTable::<16>::default();
    table.add_reading_for_tests("station1", TemperatureReading::new(123));

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
    let mut table = WeatherStationTable::<16>::default();
    table.add_reading_for_tests("station1", TemperatureReading::new(123));
    table.add_reading_for_tests("station2", TemperatureReading::new(456));

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
    let mut table = WeatherStationTable::<16>::default();
    table.add_reading_for_tests("station1", TemperatureReading::new(123));
    table.add_reading_for_tests("station1", TemperatureReading::new(456));

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
