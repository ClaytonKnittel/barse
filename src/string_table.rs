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

  fn station_hash(&self, station: &str) -> u64 {
    str_hash(station.as_bytes())
  }

  fn station_index(&self, station: &str) -> usize {
    self.station_hash(station) as usize % SIZE
  }

  fn find_entry(&mut self, station: &str) -> &mut InlineString {
    let idx = self.station_index(station);

    if likely(
      self
        .table
        .entry_at_mut(idx)
        .matches_key_or_initialize(station),
    ) {
      return self.entry_at_mut(idx);
    }

    // Otherwise we have to search for a bucket.
    self.scan_for_entry(station, idx)
  }

  pub fn index_of_station(&mut self, station: &str) {
    self.find_entry(station)
  }
}
