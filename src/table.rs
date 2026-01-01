use std::fmt::Debug;

use crate::{
  error::BarseResult, hugepage_backed_table::HugepageBackedTable, str_hash::str_hash,
  table_entry::Entry, temperature_reading::TemperatureReading,
  temperature_summary::TemperatureSummary, util::likely,
};

pub struct WeatherStationTable<const SIZE: usize> {
  table: HugepageBackedTable<Entry, SIZE>,
}

impl<const SIZE: usize> WeatherStationTable<SIZE> {
  pub fn new() -> BarseResult<Self> {
    Ok(Self {
      table: HugepageBackedTable::new()?,
    })
  }

  pub fn iter(&self) -> impl Iterator<Item = (&str, &TemperatureSummary)> {
    WeatherStationIterator {
      table: self,
      index: 0,
    }
  }

  pub fn merge(&mut self, other: Self) {
    for (station, summary) in other.iter() {
      let entry = self.find_entry(station);
      entry.merge_summary(summary);
    }
  }

  fn entry_at(&self, index: usize) -> &Entry {
    self.table.entry_at(index)
  }

  fn entry_at_mut(&mut self, index: usize) -> &mut Entry {
    self.table.entry_at_mut(index)
  }

  fn scan_for_entry(&mut self, station: &str, start_idx: usize) -> &mut Entry {
    let idx = (1..SIZE)
      .map(|i| (start_idx + i) % SIZE)
      .find(|&idx| self.entry_at_mut(idx).matches_key_or_initialize(station))
      .expect("No empty bucket found, table is full");
    self.entry_at_mut(idx)
  }

  pub fn add_reading(&mut self, station: &str, reading: TemperatureReading) {
    self.find_entry(station).add_reading(reading);
  }

  fn station_hash(&self, station: &str) -> u64 {
    str_hash(station.as_bytes())
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

  fn new_table<const SIZE: usize>() -> WeatherStationTable<SIZE> {
    WeatherStationTable::new().unwrap()
  }

  #[gtest]
  fn test_insert() {
    let mut table = new_table::<16>();
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
    let mut table = new_table::<16>();
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
    let mut table = new_table::<16>();
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

  #[gtest]
  fn test_merge() {
    let mut table1 = new_table::<16>();
    table1.add_reading("station1", TemperatureReading::new(123));
    table1.add_reading("station1", TemperatureReading::new(-456));
    table1.add_reading("station2", TemperatureReading::new(324));

    let mut table2 = new_table::<16>();
    table2.add_reading("station1", TemperatureReading::new(-100));
    table2.add_reading("station2", TemperatureReading::new(-200));
    table2.add_reading("station2", TemperatureReading::new(-300));

    table1.merge(table2);
    let elements = table1.iter().collect_vec();
    expect_that!(
      elements,
      unordered_elements_are![
        (
          eq(&"station1"),
          derefs_to(pat!(TemperatureSummary {
            min: &TemperatureReading::new(-456),
            max: &TemperatureReading::new(123),
            total: &-433,
            count: &3,
          }))
        ),
        (
          eq(&"station2"),
          derefs_to(pat!(TemperatureSummary {
            min: &TemperatureReading::new(-300),
            max: &TemperatureReading::new(324),
            total: &-176,
            count: &3,
          }))
        )
      ]
    );
  }

  #[gtest]
  fn test_merge_collisions() {
    let mut table1 = new_table::<16>();
    table1.add_reading("station station 1", TemperatureReading::new(10));
    table1.add_reading("station station 2", TemperatureReading::new(30));

    let mut table2 = new_table::<16>();
    table2.add_reading("station station 2", TemperatureReading::new(-30));
    table2.add_reading("station station 1", TemperatureReading::new(-100));

    table1.merge(table2);
    let elements = table1.iter().collect_vec();
    expect_that!(
      elements,
      unordered_elements_are![
        (
          eq(&"station station 1"),
          derefs_to(pat!(TemperatureSummary {
            min: &TemperatureReading::new(-100),
            max: &TemperatureReading::new(10),
            total: &-90,
            count: &2,
          }))
        ),
        (
          eq(&"station station 2"),
          derefs_to(pat!(TemperatureSummary {
            min: &TemperatureReading::new(-30),
            max: &TemperatureReading::new(30),
            total: &0,
            count: &2,
          }))
        )
      ]
    );
  }
}
