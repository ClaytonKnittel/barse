use std::{
  fs::File,
  hash::{self, BuildHasher, Hasher, RandomState},
  io::{BufRead, BufReader},
  process::ExitCode,
  ptr::read_unaligned,
};

use barse::{
  error::{BarseError, BarseResult},
  str_hash::BuildStringHash,
};
use rand::{rng, seq::IteratorRandom};

fn compute_hash_quality<V, H>(values: &[V], mut hash: H, buckets: usize) -> f32
where
  V: std::fmt::Display,
  H: FnMut(&V) -> u64,
{
  values
    .iter()
    .scan(vec![None; buckets], |buckets, v| {
      let h = hash(v);
      let mut bucket_idx = h as usize % buckets.len();
      let mut count = 1;
      while let Some(other_v) = buckets[bucket_idx] {
        // println!("Collision for {} with {}", v, other_v);
        bucket_idx = (bucket_idx + 1) % buckets.len();
        count += 1;
      }
      buckets[bucket_idx] = Some(v);
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

fn mask_char_and_above<const NEEDLE: u8>(v: u128) -> u128 {
  const LSB: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;
  let search_mask = (NEEDLE as u128) * LSB;
  let zeroed_needles = v ^ search_mask;
  let lsb_one_for_zeros = ((!zeroed_needles & zeroed_needles.wrapping_sub(LSB)) >> 7) & LSB;
  let keep_mask = lsb_one_for_zeros.wrapping_sub(1) & !lsb_one_for_zeros;
  v & keep_mask
}

fn compress_lower_nibbles(v: u128) -> u64 {
  // const LOWER_NIBBLE: u64 = 0x0f0f_0f0f_0f0f_0f0f;
  v as u64 ^ (v >> 64) as u64
}

fn scramble_u64(v: u64) -> u64 {
  const MAGIC: u64 = 0x20000400020001;
  v.wrapping_mul(MAGIC) >> 48
}

fn new_hash(bytes: &str) -> u64 {
  let v = unsafe { read_unaligned(bytes.as_ptr() as *const u128) };
  let v = mask_char_and_above::<b';'>(v);
  let v = compress_lower_nibbles(v);
  scramble_u64(v)
}

fn run() -> BarseResult {
  let weather_stations = weather_stations("data/weather_stations.csv")?;
  const CAP: usize = 65536;

  const BELOW: usize = 32;
  println!(
    "Pct below {BELOW}: {}",
    weather_stations
      .iter()
      .filter(|station| station.len() <= BELOW)
      .count() as f32
      / weather_stations.len() as f32
      * (4097 - BELOW) as f32
      / 4096.
  );

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
      |station| {
        let mut hasher = BuildStringHash.build_hasher();
        hasher.write(station.as_bytes());
        hasher.finish()
      },
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
