use crate::{
  error::BarseResult, hugepage_backed_table::HugepageBackedTable,
  temperature_reading::TemperatureReading, temperature_summary::TemperatureSummary,
};

pub struct TemperatureSummaryTable<const SIZE: usize> {
  table: HugepageBackedTable<TemperatureSummary, SIZE>,
}

impl<const SIZE: usize> TemperatureSummaryTable<SIZE> {
  pub fn new() -> BarseResult<Self> {
    Ok(Self {
      table: HugepageBackedTable::new()?,
    })
  }

  pub fn add_reading_at_index(&mut self, temp: TemperatureReading, index: usize) {
    self.table.entry_at_mut(index).add_reading(temp);
  }

  pub fn merge(&mut self, other: Self) {
    for i in 0..SIZE {
      self.table.entry_at_mut(i).merge(other.table.entry_at(i));
    }
  }
}
