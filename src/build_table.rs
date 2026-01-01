use crate::{
  error::BarseResult, scanner::Scanner, str_hash::TABLE_SIZE, table::WeatherStationTable,
  temperature_summary::TemperatureSummary, util::HasIter,
};

pub fn build_temperature_reading_table_from_bytes(
  input: &[u8],
) -> BarseResult<impl for<'a> HasIter<'a, Item = (&'a str, &'a TemperatureSummary)>> {
  Ok(Scanner::from_start(input).fold(
    WeatherStationTable::<TABLE_SIZE>::new()?,
    |mut map, (station, temp)| {
      map.add_reading(station, temp);
      map
    },
  ))
}
