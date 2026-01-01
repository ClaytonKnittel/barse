use crate::{
  error::BarseResult, scanner::Scanner, str_hash::TABLE_SIZE, table::WeatherStationTable,
};

pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<WeatherStationTable<TABLE_SIZE>> {
  Ok(
    Scanner::from_start(input).fold(WeatherStationTable::new()?, |mut map, (station, temp)| {
      map.add_reading(station, temp);
      map
    }),
  )
}
