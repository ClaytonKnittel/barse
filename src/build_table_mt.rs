use crate::{
  error::{BarseError, BarseResult},
  str_hash::TABLE_SIZE,
  table::WeatherStationTable,
};
use itertools::Itertools;
use std::sync::Arc;

pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
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
