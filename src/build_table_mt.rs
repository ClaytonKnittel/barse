use crate::{
  error::{BarseError, BarseResult},
  shared_table::SharedTable,
  str_hash::TABLE_SIZE,
};
use std::sync::Arc;

pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<SharedTable<TABLE_SIZE>> {
  let thread_count = std::thread::available_parallelism()
    .map(|nonzero| nonzero.get())
    .unwrap_or(1) as u32;

  let slicer = Arc::new(unsafe { crate::slicer::Slicer::new(input) });
  let shared_table = Arc::new(SharedTable::new(thread_count)?);

  let threads = (0..thread_count)
    .map(|thread_index| -> BarseResult<_> {
      let slicer = slicer.clone();
      let shared_table = shared_table.clone();
      Ok(std::thread::spawn(move || {
        while let Some(slice) = slicer.next_slice() {
          for (station, temp) in slice {
            shared_table.add_reading(station, temp, thread_index);
          }
        }
      }))
    })
    .collect::<Result<Vec<_>, _>>()?;

  for thread in threads {
    thread
      .join()
      .map_err(|err| BarseError::new(format!("Failed to join thread: {err:?}")))?;
  }

  Arc::try_unwrap(shared_table)
    .map_err(|_| BarseError::new("Failed to unwrap shared table Arc".to_owned()).into())
}
