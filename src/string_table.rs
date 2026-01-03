use crate::{
  error::BarseResult, hugepage_backed_table::HugepageBackedTable, inline_string_mt::InlineString,
  str_hash::str_hash,
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

  fn scan_for_entry(&self, station: &str, start_idx: usize) -> usize {
    (1..SIZE)
      .map(|i| (start_idx + i) % SIZE)
      .find(|&idx| self.table.entry_at(idx).eq_or_initialize(station))
      .expect("No empty bucket found, table is full")
  }

  pub fn find_entry_index(&self, station: &str) -> usize {
    let idx = self.station_index(station);
    let entry = self.entry_at(idx);
    if entry.eq_or_initialize(station) {
      idx
    } else {
      self.scan_for_entry(station, idx)
    }
  }
}
