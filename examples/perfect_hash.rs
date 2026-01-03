use std::{
  collections::HashMap,
  fs::File,
  io::{BufRead, BufReader},
  process::ExitCode,
  ptr::read_unaligned,
};

use barse::error::{BarseError, BarseResult};
use itertools::Itertools;
use rand::{rng, seq::SliceRandom};

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

fn entropy(stations: &[String], mask: u128) -> f32 {
  let mut set = HashMap::<u128, u32>::new();
  for station in stations {
    let v = unsafe { read_unaligned(station.as_ptr() as *const u128) };
    let v = mask_above(v, station.len());
    let v = v & mask;

    *set.entry(v).or_default() += 1;
  }

  set
    .into_values()
    .map(|count| count as f32 / stations.len() as f32)
    .map(|p| -p * p.log2())
    .sum()
}

#[derive(PartialEq, PartialOrd)]
struct F32Ord(f32);

impl Eq for F32Ord {}

#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for F32Ord {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.0.partial_cmp(&other.0).unwrap()
  }
}

fn find_bits(stations: &[String]) -> u128 {
  const TABLE_BITS: u32 = 17;

  let mut rng = rng();
  let mut bits = u128::MAX;

  let mut order = (0..u128::BITS).collect_vec();
  let cur_entropy = entropy(stations, bits);

  // Remove the obviously unhelpful bits.
  order
    .extract_if(.., |&mut bit| {
      let mask = bits & !(1 << bit);
      if entropy(stations, mask) >= cur_entropy - 0.01 {
        bits = mask;
        true
      } else {
        false
      }
    })
    .count();

  while order.len() > TABLE_BITS as usize {
    order.shuffle(&mut rng);
    let to_remove = order
      .iter()
      .cloned()
      .max_by_key(|&bit| {
        let mask = bits & !(1 << bit);
        F32Ord(entropy(stations, mask))
      })
      .unwrap();
    bits &= !(1 << to_remove);
    println!(
      "Removed bit {to_remove}, entropy now: {}",
      entropy(stations, bits)
    );
    order.swap_remove(
      order
        .iter()
        .find_position(|&&bit| bit == to_remove)
        .unwrap()
        .0,
    );
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
