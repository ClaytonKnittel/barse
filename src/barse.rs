use std::{fs::File, slice};

use memmap2::{Advice, MmapOptions};

use crate::{
  error::BarseResult, scanner::Scanner, str_hash::BuildStringHash, table::WeatherStationTable,
};

unsafe fn round_up_to_32b_boundary(buffer: &[u8]) -> &[u8] {
  unsafe { slice::from_raw_parts(buffer.as_ptr(), buffer.len().next_multiple_of(32)) }
}

pub fn build_temperature_reading_table(
  input_path: &str,
) -> BarseResult<WeatherStationTable<16384, BuildStringHash>> {
  let file = File::open(input_path)?;
  let map = unsafe { MmapOptions::new().map(&file) }?;
  map.advise(Advice::Sequential)?;
  let map_buffer = unsafe { round_up_to_32b_boundary(&map) };

  Scanner::new(map_buffer).try_fold(
    WeatherStationTable::with_hasher(BuildStringHash),
    |mut map, (station, temp)| -> BarseResult<_> {
      map.add_reading(station, temp);
      Ok(map)
    },
  )
}
