use std::{fs::File, slice};

use memmap2::{Advice, MmapOptions};

use crate::{error::BarseResult, scanner::Scanner, table::WeatherStationTable};

const TABLE_SIZE: usize = 65536;

unsafe fn round_up_to_32b_boundary(buffer: &[u8]) -> &[u8] {
  unsafe { slice::from_raw_parts(buffer.as_ptr(), buffer.len().next_multiple_of(32)) }
}

pub fn build_temperature_reading_table(
  input_path: &str,
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
  let file = File::open(input_path)?;
  let map = unsafe { MmapOptions::new().map(&file) }?;
  map.advise(Advice::Sequential)?;
  let map_buffer = unsafe { round_up_to_32b_boundary(&map) };

  Scanner::new(map_buffer).try_fold(
    WeatherStationTable::new(),
    |mut map, (station, hash, temp)| -> BarseResult<_> {
      map.add_reading(station, hash, temp);
      Ok(map)
    },
  )
}
