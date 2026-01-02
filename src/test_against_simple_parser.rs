use std::{cmp::Ordering, collections::HashMap, fmt::Display};

use crate::{test_util::random_input_file, util::HasIter};
use googletest::prelude::*;
use itertools::Itertools;

#[cfg(not(feature = "multithreaded"))]
use crate::build_table::build_temperature_reading_table_from_bytes;
#[cfg(feature = "multithreaded")]
use crate::build_table_mt::build_temperature_reading_table_from_bytes;

struct TemperatureSummary {
  min: i32,
  max: i32,
  total: i64,
  count: u32,
}

impl TemperatureSummary {
  fn min(&self) -> f32 {
    self.min as f32 / 10.0
  }

  fn max(&self) -> f32 {
    self.max as f32 / 10.0
  }

  fn avg(&self) -> f32 {
    let rounded_total = self.total + (self.count / 2) as i64;
    rounded_total.div_euclid(self.count as i64) as f32 / 10.0
  }

  fn add_reading(&mut self, temp: f32) {
    let temp = (temp * 10.0).round() as i32;
    self.min = self.min.min(temp);
    self.max = self.max.max(temp);
    self.total += temp as i64;
    self.count += 1;
  }
}

impl Default for TemperatureSummary {
  fn default() -> Self {
    Self {
      min: i32::MAX,
      max: i32::MIN,
      total: 0,
      count: 0,
    }
  }
}

pub struct WeatherStation {
  name: String,
  summary: TemperatureSummary,
}

impl PartialEq for WeatherStation {
  fn eq(&self, other: &Self) -> bool {
    self.name.eq(&other.name)
  }
}

impl Eq for WeatherStation {}

impl PartialOrd for WeatherStation {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for WeatherStation {
  fn cmp(&self, other: &Self) -> Ordering {
    self.name.cmp(&other.name)
  }
}

impl Display for WeatherStation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}={:.1}/{:.1}/{:.1}",
      self.name,
      self.summary.min(),
      self.summary.avg(),
      self.summary.max()
    )
  }
}

fn expected_temperature_reading_summaries(input_bytes: &str) -> impl Iterator<Item = String> {
  input_bytes
    .split('\n')
    .filter(|line| !line.is_empty())
    .fold(
      HashMap::<String, TemperatureSummary>::new(),
      |mut map, line| {
        let (station, temp) = line
          .split_once(';')
          .unwrap_or_else(|| panic!("No ';' found in \"{line}\""));
        map
          .entry(station.to_owned())
          .or_default()
          .add_reading(temp.parse().unwrap());
        map
      },
    )
    .into_iter()
    .map(|(station, summary)| WeatherStation {
      name: station,
      summary,
    })
    .sorted_unstable()
    .map(|station| format!("{station}"))
}

fn barse_temperature_reading_summaries(input_bytes: &[u8]) -> impl Iterator<Item = String> {
  build_temperature_reading_table_from_bytes(input_bytes)
    .unwrap()
    .iter()
    .map(|(station, summary)| crate::barse::WeatherStation::new(station, summary))
    .sorted_unstable()
    .map(|station| format!("{station}"))
    .collect_vec()
    .into_iter()
}

fn assert_equal_outputs<I1, I2>(iter1: I1, iter2: I2)
where
  I1: IntoIterator<Item = String>,
  I2: IntoIterator<Item = String>,
{
  let mut iter1 = iter1.into_iter();
  let mut iter2 = iter2.into_iter();
  for line_no in 0.. {
    match (iter1.next(), iter2.next()) {
      (Some(next1), Some(next2)) => {
        if next1 != next2 {
          panic!("Line {line_no}: \"{next1}\" != \"{next2}\"");
        }
      }
      (None, None) => break,
      (Some(next1), None) => {
        panic!("Unexpected line after end of expected input (line {line_no}): \"{next1}\"")
      }
      (None, Some(next2)) => {
        panic!("Unexpected end of output, missing line (line {line_no}): \"{next2}\"")
      }
    }
  }
}

#[gtest]
fn test_fuzz_10_000_x_10() {
  let input = random_input_file(0x12312312, 10_000, 10).unwrap();
  assert_equal_outputs(
    barse_temperature_reading_summaries(input.padded_slice()),
    expected_temperature_reading_summaries(str::from_utf8(input.exact_slice()).unwrap()),
  );
}

#[gtest]
fn test_fuzz_100_000_x_100() {
  let input = random_input_file(0x43f9e1, 100_000, 100).unwrap();
  assert_equal_outputs(
    barse_temperature_reading_summaries(input.padded_slice()),
    expected_temperature_reading_summaries(str::from_utf8(input.exact_slice()).unwrap()),
  );
}

#[gtest]
#[ignore]
fn test_fuzz_10_000_000_x_10_000() {
  let input = random_input_file(0x09f8eab1, 10_000_000, 10_000).unwrap();
  assert_equal_outputs(
    barse_temperature_reading_summaries(input.padded_slice()),
    expected_temperature_reading_summaries(str::from_utf8(input.exact_slice()).unwrap()),
  );
}
