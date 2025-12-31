use core::f32;
use std::{
  collections::HashSet,
  fs::File,
  io::{BufRead, BufReader},
  process::ExitCode,
};

use barse::error::{BarseError, BarseResult};
use itertools::Itertools;
use rand::{rng, seq::IteratorRandom};

const TABLE_SHIFT: u32 = 15;
const TABLE_SIZE: usize = 1 << TABLE_SHIFT;

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
      .collect::<Result<HashSet<_>, _>>()?
      .into_iter()
      .choose_multiple(&mut rng, 10_000),
  )
}

fn mask_above(v: u128, len: usize) -> u128 {
  v & 1u128.unbounded_shl(8 * len.min(16) as u32).wrapping_sub(1)
}

fn scramble_u64(v: u64, p: u64) -> u64 {
  v.wrapping_mul(p) >> (64 - TABLE_SHIFT)
}

fn new_hash(bytes: &str, p: u64) -> u64 {
  let mut local_hash = [0u64; 4];
  for (dst, chunk) in local_hash
    .iter_mut()
    .zip(bytes.as_bytes().chunks(std::mem::size_of::<u64>()))
  {
    if chunk.len() == 8 {
      *dst = u64::from_ne_bytes(chunk.try_into().unwrap());
    } else {
      for (i, c) in chunk.iter().enumerate() {
        *dst += (*c as u64) << (8 * i);
      }
    }
  }
  for hash in &mut local_hash {
    *hash = scramble_u64(*hash, p);
  }
  local_hash[0] ^ local_hash[1] ^ local_hash[2] ^ local_hash[3]
}

fn run() -> BarseResult {
  let weather_stations = weather_stations("data/weather_stations.csv")?;

  fn nicely_spread(magic: u64) -> bool {
    (magic & (magic >> 1)) == 0
      && (magic & (magic >> 2)) == 0
      && (magic & (magic >> 3)) == 0
      && (magic & (magic >> 4)) == 0
      && (magic & (magic >> 5)) == 0
  }

  let mut best_quality = f32::MAX;
  for p in (0..64)
    .tuple_combinations()
    .map(|(b1, b2, b3, b4)| (1 << b1) | (1 << b2) | (1 << b3) | (1 << b4))
    .filter(|p| nicely_spread(*p))
  {
    let hash_fn = |bytes: &String| new_hash(bytes, p);
    let quality = compute_hash_quality(&weather_stations, hash_fn, TABLE_SIZE);
    if quality < best_quality {
      best_quality = quality;
      println!("Quality {quality} for {p:08x}");
    }
  }
  for p in (0..64)
    .tuple_combinations()
    .map(|(b1, b2, b3, b4, b5)| (1 << b1) | (1 << b2) | (1 << b3) | (1 << b4) | (1 << b5))
    .filter(|p| nicely_spread(*p))
  {
    let hash_fn = |bytes: &String| new_hash(bytes, p);
    let quality = compute_hash_quality(&weather_stations, hash_fn, TABLE_SIZE);
    if quality < best_quality {
      best_quality = quality;
      println!("Quality {quality} for {p:08x}");
    }
  }
  for p in (0..64)
    .tuple_combinations()
    .map(|(b1, b2, b3, b4, b5, b6)| {
      (1 << b1) | (1 << b2) | (1 << b3) | (1 << b4) | (1 << b5) | (1 << b6)
    })
    .filter(|p| nicely_spread(*p))
  {
    let hash_fn = |bytes: &String| new_hash(bytes, p);
    let quality = compute_hash_quality(&weather_stations, hash_fn, TABLE_SIZE);
    if quality < best_quality {
      best_quality = quality;
      println!("Quality {quality} for {p:08x}");
    }
  }
  for p in (0..64)
    .tuple_combinations()
    .map(|(b1, b2, b3, b4, b5, b6, b7)| {
      (1 << b1) | (1 << b2) | (1 << b3) | (1 << b4) | (1 << b5) | (1 << b6) | (1 << b7)
    })
    .filter(|p| nicely_spread(*p))
  {
    let hash_fn = |bytes: &String| new_hash(bytes, p);
    let quality = compute_hash_quality(&weather_stations, hash_fn, TABLE_SIZE);
    if quality < best_quality {
      best_quality = quality;
      println!("Quality {quality} for {p:08x}");
    }
  }
  for p in (0..64)
    .tuple_combinations()
    .map(|(b1, b2, b3, b4, b5, b6, b7, b8)| {
      (1 << b1) | (1 << b2) | (1 << b3) | (1 << b4) | (1 << b5) | (1 << b6) | (1 << b7) | (1 << b8)
    })
    .filter(|p| nicely_spread(*p))
  {
    let hash_fn = |bytes: &String| new_hash(bytes, p);
    let quality = compute_hash_quality(&weather_stations, hash_fn, TABLE_SIZE);
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
