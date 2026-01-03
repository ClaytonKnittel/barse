use std::{
  collections::{HashMap, HashSet},
  fs::File,
  io::{BufRead, BufReader},
  process::ExitCode,
  ptr::read_unaligned,
};

use barse::error::{BarseError, BarseResult};

fn all_weather_stations(path: &str) -> BarseResult<Vec<String>> {
  Ok(
    BufReader::new(File::open(path)?)
      .lines()
      .filter(|line| !line.as_ref().is_ok_and(|line| line.starts_with('#')))
      .map(|line| -> BarseResult<_> {
        let line = line?;
        line
          .split_once(';')
          .ok_or_else(|| BarseError::new(format!("No ';' found in line \"{line}\"")).into())
          .map(|(station, _)| {
            (
              unsafe { str::from_utf8_unchecked(&station.as_bytes()[..station.len().min(16)]) }
                .to_owned(),
              station.to_owned(),
            )
          })
      })
      .collect::<Result<HashMap<_, _>, _>>()?
      .into_values()
      .collect(),
  )
}

fn mask_above(v: u128, len: usize) -> u128 {
  v & 1u128.unbounded_shl(8 * len.min(16) as u32).wrapping_sub(1)
}

fn unique_with_mask(stations: &[String], mask: u128) -> bool {
  let mut set = HashSet::new();
  for station in stations {
    let v = unsafe { read_unaligned(station.as_ptr() as *const u128) };
    let v = mask_above(v, station.len());
    let v = v & mask;

    if !set.insert(v) {
      return false;
    }
  }

  true
}

fn find_bits(stations: &[String]) -> u128 {
  let mut bits = u128::MAX;

  debug_assert!(unique_with_mask(stations, bits));
  for bit in 0..u128::BITS {
    let mask = bits & !(1 << bit);
    if unique_with_mask(stations, mask) {
      bits = mask;
    }
  }

  bits
}

fn run() -> BarseResult {
  let weather_stations = all_weather_stations("data/weather_stations.csv")?;

  let bits = find_bits(&weather_stations);
  println!("Bits: {bits:032x} ({} set)", bits.count_ones());

  Ok(())
}

fn main() -> ExitCode {
  if let Err(err) = run() {
    println!("{err}");
    ExitCode::FAILURE
  } else {
    ExitCode::SUCCESS
  }
}
