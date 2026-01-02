use crate::{
  error::BarseResult, hugepage_backed_table::HugepageBackedTable, inline_string::InlineString,
  str_hash::str_hash, util::likely,
};

pub struct StringTable<const SIZE: usize> {
  table: HugepageBackedTable<InlineString, SIZE>,
}

impl<const SIZE: usize> StringTable<SIZE> {
  pub fn new() -> BarseResult<Self> {
    Ok(Self {
      table: HugepageBackedTable::new()?,
    })
  }

  pub fn entry_at(&self, index: usize) -> &InlineString {
    self.table.entry_at(index)
  }

  fn station_hash(&self, station: &str) -> u64 {
    str_hash(station.as_bytes())
  }

  fn station_index(&self, station: &str) -> usize {
    self.station_hash(station) as usize % SIZE
  }

  fn eq_or_initialize(entry: &InlineString, station: &str) -> bool {
    if likely(entry.initialized()) {
      likely(entry.eq_foreign_str(station))
    } else {
      entry.try_initialize(station)
    }
  }

  fn scan_for_entry(&self, station: &str, start_idx: usize) -> usize {
    (1..SIZE)
      .map(|i| (start_idx + i) % SIZE)
      .find(|&idx| Self::eq_or_initialize(self.table.entry_at(idx), station))
      .expect("No empty bucket found, table is full")
  }

  pub fn find_entry_index(&self, station: &str) -> usize {
    let idx = self.station_index(station);
    let entry = self.entry_at(idx);
    if Self::eq_or_initialize(entry, station) {
      idx
    } else {
      self.scan_for_entry(station, idx)
    }
  }
}
