use crate::{
  error::{BarseError, BarseResult},
  str_hash::TABLE_SIZE,
  string_table::StringTable,
  temperature_summary::TemperatureSummary,
  temperature_summary_table::TemperatureSummaryTable,
  util::HasIter,
};
use std::sync::Arc;

pub struct SummaryTable<const SIZE: usize> {
  string_table: Arc<StringTable<SIZE>>,
  temp_table: TemperatureSummaryTable<SIZE>,
}

impl<'a, const SIZE: usize> HasIter<'a> for SummaryTable<SIZE> {
  type Item = (&'a str, &'a TemperatureSummary);

  fn iter(&'a self) -> impl Iterator<Item = Self::Item> {
    (0..SIZE).filter_map(|i| {
      let station = self.string_table.entry_at(i);
      station
        .initialized()
        .then(|| (station.value_str(), self.temp_table.entry_at(i)))
    })
  }
}

pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<SummaryTable<TABLE_SIZE>> {
  let thread_count = std::thread::available_parallelism()
    .map(|nonzero| nonzero.get())
    .unwrap_or(1);

  let slicer = Arc::new(unsafe { crate::slicer::Slicer::new(input) });
  let string_table = Arc::new(StringTable::new()?);

  let mut threads = (0..thread_count)
    .map(|_| -> BarseResult<_> {
      let slicer = slicer.clone();
      let string_table = string_table.clone();
      let mut summary_table = TemperatureSummaryTable::new()?;
      Ok(std::thread::spawn(move || {
        while let Some(slice) = slicer.next_slice() {
          for (station, temp) in slice {
            let idx = string_table.find_entry_index(station);
            summary_table.add_reading_at_index(temp, idx);
          }
        }
        summary_table
      }))
    })
    .collect::<Result<Vec<_>, _>>()?;

  let mut temp_table = threads
    .pop()
    .expect("Thread list will not be empty")
    .join()
    .map_err(|err| BarseError::new(format!("Failed to join thread: {err:?}")))?;

  for thread in threads {
    let thread_map = thread
      .join()
      .map_err(|err| BarseError::new(format!("Failed to join thread: {err:?}")))?;
    temp_table.merge(thread_map);
  }

  Ok(SummaryTable {
    string_table,
    temp_table,
  })
}
