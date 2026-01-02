use itertools::Itertools;

use crate::{
  barse::{build_temperature_reading_table, WeatherStation},
  error::BarseResult,
  util::HasIter,
};

pub fn print_summary(input_path: &str) -> BarseResult {
  println!(
    "{{{}}}",
    build_temperature_reading_table(input_path)?
      .iter()
      .map(|(station, summary)| WeatherStation::new(station, summary))
      .sorted_unstable()
      .map(|station| format!("{station}"))
      .join(", ")
  );
  Ok(())
}
