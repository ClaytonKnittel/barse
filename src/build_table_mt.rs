use crate::{
  error::{BarseError, BarseResult},
  str_hash::TABLE_SIZE,
  table::WeatherStationTable,
};
use std::sync::Arc;

pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
  let thread_count = std::thread::available_parallelism()
    .map(|nonzero| nonzero.get())
    .unwrap_or(1);

  let slicer = Arc::new(unsafe { crate::slicer::Slicer::new(input) });

  let mut threads = (0..thread_count)
    .map(|_| -> BarseResult<_> {
      let slicer = slicer.clone();
      let mut map = WeatherStationTable::<TABLE_SIZE>::new()?;
      Ok(std::thread::spawn(move || {
        while let Some(slice) = slicer.next_slice() {
          for (station, temp) in slice {
            map.add_reading(station, temp);
          }
        }
        map
      }))
    })
    .collect::<Result<Vec<_>, _>>()?;

  let mut map = threads
    .pop()
    .expect("Thread list will not be empty")
    .join()
    .map_err(|err| BarseError::new(format!("Failed to join thread: {err:?}")))?;

  for thread in threads {
    let thread_map = thread
      .join()
      .map_err(|err| BarseError::new(format!("Failed to join thread: {err:?}")))?;
    map.merge(thread_map);
  }

  Ok(map)
}
