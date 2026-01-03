use crate::{
  error::BarseResult,
  hugepage_backed_table::HugepageBackedTable,
  inline_string_mt::InlineString,
  str_hash::str_hash,
  temperature_reading::TemperatureReading,
  temperature_summary_mt::TemperatureSummary,
  util::{HasIter, InPlaceInitializable},
};

struct Entry {
  key: InlineString,
  temp_summary: TemperatureSummary,
}

impl Entry {
  fn initialized(&self) -> bool {
    self.key.initialized()
  }
}

impl InPlaceInitializable for Entry {
  fn initialize(&mut self) {
    self.key.initialize();
    self.temp_summary.initialize();
  }
}

pub struct SharedTable<const SIZE: usize> {
  table: HugepageBackedTable<Entry, SIZE>,
}

impl<const SIZE: usize> SharedTable<SIZE> {
  pub fn new() -> BarseResult<Self> {
    let table = HugepageBackedTable::new()?;
    Ok(Self { table })
  }

  #[allow(clippy::mut_from_ref)]
  fn entry_at(&self, index: usize) -> &Entry {
    self.table.entry_at(index)
  }

  fn station_hash(&self, station: &str) -> u64 {
    str_hash(station.as_bytes())
  }

  fn station_index(&self, station: &str) -> usize {
    self.station_hash(station) as usize % SIZE
  }

  fn scan_for_entry(&self, station: &str, start_idx: usize) -> &TemperatureSummary {
    (1..SIZE)
      .map(|i| (start_idx + i) % SIZE)
      .map(|idx| self.entry_at(idx))
      .find(|entry| entry.key.eq_or_initialize(station))
      .map(|entry| &entry.temp_summary)
      .expect("No empty bucket found, table is full")
  }

  fn find_entry(&self, station: &str) -> &TemperatureSummary {
    let idx = self.station_index(station);
    let entry = self.entry_at(idx);
    if entry.key.eq_or_initialize(station) {
      &entry.temp_summary
    } else {
      self.scan_for_entry(station, idx)
    }
  }

  pub fn add_reading(&self, station: &str, temp: TemperatureReading) {
    let temp_summary = self.find_entry(station);
    temp_summary.add_reading(temp);
  }
}

impl<'a, const SIZE: usize> HasIter<'a> for SharedTable<SIZE> {
  type Item = (&'a str, TemperatureSummary);

  fn iter(&'a self) -> impl Iterator<Item = Self::Item> {
    (0..SIZE)
      .map(|index| self.entry_at(index))
      .filter(|entry| entry.initialized())
      .map(|entry| (entry.key.value_str(), entry.temp_summary.clone()))
  }
}
