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

fn new_hash(bytes: &str) -> u64 {
  let to_u32: u32 = bytes
    .as_bytes()
    .iter()
    .take(8)
    .enumerate()
    .map(|(i, b)| ((b & 0x0f) as u32) << (4 * i))
    .sum();
  (to_u32.wrapping_mul(0x01008021) >> 16) as u64
}

fn run() -> BarseResult {
  let weather_stations = weather_stations("data/weather_stations.csv")?;
  const CAP: usize = 65536;

  println!(
    "Default hash quality: {}",
    compute_hash_quality(
      &weather_stations,
      |station| {
        let mut hasher = hash::DefaultHasher::new();
        hasher.write(station.as_bytes());
        hasher.finish()
      },
      CAP
    )
  );

  println!(
    "RandomState hash quality: {}",
    compute_hash_quality(
      &weather_stations,
      |station| { RandomState::new().hash_one(station) },
      CAP
    )
  );

  println!(
    "My hash quality: {}",
    compute_hash_quality(
      &weather_stations,
      |station| { BuildStringHash.hash_one(station) },
      CAP
    )
  );

  println!(
    "New hash quality: {}",
    compute_hash_quality(&weather_stations, |station| { new_hash(station) }, CAP)
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
