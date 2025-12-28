use core::f32;
use std::{
  fs::File,
  io::{BufRead, BufReader},
  process::ExitCode,
  ptr::read_unaligned,
};

use barse::error::{BarseError, BarseResult};
use itertools::Itertools;
use rand::{rng, seq::IteratorRandom};

fn compute_hash_quality<V, H>(values: &[V], mut hash: H, buckets: usize) -> f32
where
  H: FnMut(&V) -> u64,
{
  values
    .iter()
    .scan(vec![false; buckets], |buckets, v| {
      let h = hash(v);
      let mut bucket_idx = h as usize % buckets.len();
      let mut count = 1;
      while buckets[bucket_idx] {
        bucket_idx = (bucket_idx + 1) % buckets.len();
        count += 1;
      }
      buckets[bucket_idx] = true;
      Some(count)
    })
    .sum::<u64>() as f32
    / values.len() as f32
}

fn weather_stations(path: &str) -> BarseResult<Vec<String>> {
  let mut rng = rng();
  Ok(
    BufReader::new(File::open(path)?)
      .lines()
      .filter(|line| !line.as_ref().is_ok_and(|line| line.starts_with('#')))
      .map(|line| -> BarseResult<_> {
        let line = line?;
        line
          .split_once(';')
          .ok_or_else(|| BarseError::new(format!("No ';' found in line \"{line}\"")).into())
          .map(|(station, _)| station.to_owned())
      })
      .collect::<Result<Vec<_>, _>>()?
      .into_iter()
      .choose_multiple(&mut rng, 10_000),
  )
}

fn nibble_mush(bytes: &str) -> u32 {
  bytes
    .as_bytes()
    .iter()
    .take(8)
    .enumerate()
    .map(|(i, b)| ((b & 0x0f) as u32) << (4 * i))
    .sum()
}

fn mask_char_and_above<const NEEDLE: u8>(v: u128) -> u128 {
  const LSB: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;
  let search_mask = (NEEDLE as u128) * LSB;
  let zeroed_needles = v ^ search_mask;
  let lsb_one_for_zeros = ((!zeroed_needles & zeroed_needles.wrapping_sub(LSB)) >> 7) & LSB;
  let keep_mask = lsb_one_for_zeros.wrapping_sub(1) & !lsb_one_for_zeros;
  v & keep_mask
}

fn scramble_u64(v: u64, p: u64) -> u64 {
  v.wrapping_mul(p) >> 48
}

fn new_hash(bytes: &str, p: u64) -> u64 {
  let v = unsafe { read_unaligned(bytes.as_ptr() as *const u128) };
  let v = mask_char_and_above::<b';'>(v);
  let v = v as u64 ^ (v >> 64) as u64;
  scramble_u64(v, p) as u64
}

fn run() -> BarseResult {
  let weather_stations = weather_stations("data/weather_stations.csv")?;
  const CAP: usize = 65536;

  let mut best_quality = f32::MAX;
  for (b1, b2, b3, b4) in (0..64).tuple_combinations() {
    let p = (1 << b1) | (1 << b2) | (1 << b3) | (1 << b4);
    let hash_fn = |bytes: &String| new_hash(bytes, p);
    let quality = compute_hash_quality(&weather_stations, hash_fn, CAP);
    if quality < best_quality {
      best_quality = quality;
      println!("Quality {quality} for {p:08x}");
    }
  }

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
