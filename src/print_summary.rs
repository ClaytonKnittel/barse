use std::{cmp::Ordering, fmt::Display};

use itertools::Itertools;

use crate::{
  barse::build_temperature_reading_table, error::BarseResult, table::TemperatureSummary,
};

pub struct WeatherStation<'a> {
  name: &'a str,
  summary: TemperatureSummary,
}

impl<'a> PartialEq for WeatherStation<'a> {
  fn eq(&self, other: &Self) -> bool {
    self.name.eq(other.name)
  }
}

impl<'a> Eq for WeatherStation<'a> {}

impl<'a> PartialOrd for WeatherStation<'a> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<'a> Ord for WeatherStation<'a> {
  fn cmp(&self, other: &Self) -> Ordering {
    self.name.cmp(&other.name)
  }
}

impl<'a> Display for WeatherStation<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}={}/{}/{}",
      self.name,
      self.summary.min(),
      self.summary.avg(),
      self.summary.max()
    )
  }
}

pub fn print_summary(input_path: &str) -> BarseResult {
  println!(
    "{{{}}}",
    build_temperature_reading_table(input_path)?
      .iter()
      .map(|(station, summary)| WeatherStation {
        name: station,
        summary: *summary,
      })
      .sorted_unstable()
      .map(|station| format!("{station}"))
      .join(", ")
  );
  Ok(())
}
