use std::{
  collections::HashMap,
  fs::File,
  io::{BufRead, BufReader},
  process::ExitCode,
  ptr::read_unaligned,
};

use barse::error::{BarseError, BarseResult};
use itertools::Itertools;
use rand::{rng, seq::SliceRandom, RngCore};

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

fn entropy(stations: &[u128], mask: u128) -> f32 {
  let mut set = HashMap::<u128, u32>::new();
  for v in stations {
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

fn highest_entropy_mask(stations: &[u128]) -> u128 {
  const TABLE_BITS: u32 = 15;

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
    // println!(
    //   "Removed bit {to_remove}, entropy now: {}",
    //   entropy(stations, bits)
    // );
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

fn mul_hi(a: u64, b: u64) -> u64 {
  ((a as u128 * b as u128) >> 64) as u64
}

fn scramble(v: u128) -> u128 {
  let v = v ^ (v >> 17);
  v ^ (v >> 29)
}

fn find_bits(stations: &[String]) {
  let station_vals = stations
    .iter()
    .map(|station| {
      let v = unsafe { read_unaligned(station.as_ptr() as *const u128) };
      mask_above(v, station.len())
    })
    .collect_vec();

  let mut highest_entropy = f32::MIN;
  for _ in 0..32 {
    let mut rng = rng();
    let rand_p1 = rng.next_u64();
    let rand_p2 = rng.next_u64();
    let scrambled_stations = station_vals
      .iter()
      .map(|&station| {
        let x1 = mul_hi(station as u64, rand_p1);
        let x2 = mul_hi((station >> 64) as u64, rand_p2);
        (x1 as u128) + ((x2 as u128) << 64)
      })
      .collect_vec();

    let mask = highest_entropy_mask(&scrambled_stations);
    let e = entropy(&scrambled_stations, mask);
    if e > highest_entropy {
      println!(
    "(p1 = {rand_p1:016x}, p2 = {rand_p2:016x}): mask = {mask:032x} ({} bits set) - entropy = {e}",
    mask.count_ones()
  );
      highest_entropy = e;
    }
  }
}

fn run() -> BarseResult {
  let weather_stations = all_weather_stations("data/weather_stations.csv")?;

  find_bits(&weather_stations);

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
