use core::f32;
use std::{
  cmp::Ordering,
  collections::HashMap,
  fmt::Display,
  fs::File,
  io::{BufRead, BufReader},
  process::ExitCode,
};

use itertools::Itertools;

use crate::error::{BarseError, BarseResult};

mod error;

struct TemperatureSummary {
  min: f32,
  max: f32,
  total: f32,
  count: u32,
}

impl TemperatureSummary {
  fn min(&self) -> f32 {
    self.min
  }

  fn max(&self) -> f32 {
    self.max
  }

  fn avg(&self) -> f32 {
    self.total / self.count as f32
  }

  fn add_reading(&mut self, temp: f32) {
    self.min = self.min.min(temp);
    self.max = self.max.max(temp);
    self.total += temp;
    self.count += 1;
  }
}

impl Default for TemperatureSummary {
  fn default() -> Self {
    Self {
      min: f32::MAX,
      max: f32::MIN,
      total: 0.,
      count: 0,
    }
  }
}

struct WeatherStation {
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

fn rows(input_path: &str) -> BarseResult<impl Iterator<Item = WeatherStation>> {
  Ok(
    BufReader::new(File::open(input_path)?)
      .lines()
      .try_fold(
        HashMap::<String, TemperatureSummary>::new(),
        |mut map, line| -> BarseResult<_> {
          let line = line?;
          let (station, temp) = line
            .split_once(';')
            .ok_or_else(|| BarseError::new(format!("No ';' found in \"{line}\"")))?;
          map
            .entry(station.to_owned())
            .or_default()
            .add_reading(temp.parse()?);
          Ok(map)
        },
      )?
      .into_iter()
      .map(|(station, summary)| WeatherStation {
        name: station,
        summary,
      })
      .sorted_unstable(),
  )
}

fn run() -> BarseResult {
  const INPUT: &str = "measurements.txt";
  println!(
    "{{{}}}",
    rows(INPUT)?.map(|station| format!("{station}")).join(", ")
  );
  Ok(())
}

fn main() -> ExitCode {
  #[cfg(feature = "profiled")]
  let guard = pprof::ProfilerGuardBuilder::default()
    .frequency(1000)
    .blocklist(&["libc", "libgcc", "pthread", "vdso"])
    .build()
    .unwrap();

  let res = run();

  #[cfg(feature = "profiled")]
  if let Ok(report) = guard.report().build() {
    let file = std::fs::File::create("brc.svg").unwrap();
    report.flamegraph(file).unwrap();
  };

  if let Err(err) = res {
    println!("{err}");
    ExitCode::FAILURE
  } else {
    ExitCode::SUCCESS
  }
}
