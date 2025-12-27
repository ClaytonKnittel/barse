use std::{
  fs::File,
  hash::{self, BuildHasher, Hasher, RandomState},
  io::{BufRead, BufReader},
  process::ExitCode,
};

use barse::{
  error::{BarseError, BarseResult},
  str_hash::BuildStringHash,
};
use rand::{
  rng,
  seq::{IteratorRandom, SliceRandom},
};

fn compute_hash_quality<V, H>(values: &[V], mut hash: H, buckets: usize) -> f32
where
  H: FnMut(&V) -> u64,
{
  let denom = (values.len() * (values.len() + 2 * buckets - 1)) as f32 / (2 * buckets) as f32;
  values
    .iter()
    .map(|v| hash(v) as usize % buckets)
    .fold(vec![0; buckets], |mut buckets, bucket_idx| {
      buckets[bucket_idx] += 1;
      buckets
    })
    .into_iter()
    .map(|bucket_count| bucket_count * (bucket_count + 1) / 2)
    .sum::<u64>() as f32
    / denom
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

fn new_hash(bytes: &str) -> u64 {
  const PRODUCTS: [u8; 32] = [
    23, 167, 13, 139, 109, 107, 59, 61, 223, 179, 3, 103, 43, 149, 233, 11, 37, 101, 97, 251, 229,
    83, 157, 131, 53, 163, 241, 73, 197, 19, 17, 191,
  ];
  bytes
    .as_bytes()
    .iter()
    .zip(PRODUCTS.iter().cycle())
    .map(|(b, p)| (b ^ p) as u64)
    .fold(1, |acc, h| acc.wrapping_add(h))
}

fn run() -> BarseResult {
  let weather_stations = weather_stations("data/weather_stations.csv")?;
  let cap = (10_000usize * 8 / 7).next_power_of_two();

  println!(
    "Default hash quality: {}",
    compute_hash_quality(
      &weather_stations,
      |station| {
        let mut hasher = hash::DefaultHasher::new();
        hasher.write(station.as_bytes());
        hasher.finish()
      },
      cap
    )
  );

  println!(
    "RandomState hash quality: {}",
    compute_hash_quality(
      &weather_stations,
      |station| { RandomState::new().hash_one(station) },
      cap
    )
  );

  println!(
    "My hash quality: {}",
    compute_hash_quality(
      &weather_stations,
      |station| { BuildStringHash.hash_one(station) },
      cap
    )
  );

  println!(
    "New hash quality: {}",
    compute_hash_quality(&weather_stations, |station| { new_hash(station) }, cap)
  );
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
