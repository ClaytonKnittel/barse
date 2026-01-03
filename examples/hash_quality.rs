use std::{
  fs::File,
  hash::{self, BuildHasher, Hasher, RandomState},
  io::{BufRead, BufReader},
  process::ExitCode,
  ptr::read_unaligned,
};

use barse::{
  error::{BarseError, BarseResult},
  str_hash::str_hash,
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
      while let Some(_other_v) = buckets[bucket_idx] {
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

fn mask_above(v: u128, len: usize) -> u128 {
  v & 1u128.unbounded_shl(8 * len.min(16) as u32).wrapping_sub(1)
}

fn compress_lower_nibbles(v: u128) -> u64 {
  // const LOWER_NIBBLE: u64 = 0x0f0f_0f0f_0f0f_0f0f;
  v as u64 ^ (v >> 64) as u64
}

fn scramble_u64(v: u64) -> u64 {
  const MAGIC: u64 = 0x800800400001;
  v.wrapping_mul(MAGIC) >> 47
}

fn new_hash(bytes: &str) -> u64 {
  let v = unsafe { read_unaligned(bytes.as_ptr() as *const u128) };
  let v = mask_above(v, bytes.len());
  let v = compress_lower_nibbles(v);
  scramble_u64(v)
}

fn compress(mut x: u128, mut m: u128) -> u128 {
  x &= m; // Clear irrelevant bits.
  let mut mk = !m << 1; // We will count 0's to right.
  for i in 0..7 {
    let mp = mk ^ (mk << 1); // Parallel suffix.
    let mp = mp ^ (mp << 2);
    let mp = mp ^ (mp << 4);
    let mp = mp ^ (mp << 8);
    let mp = mp ^ (mp << 16);
    let mp = mp ^ (mp << 32);
    let mp = mp ^ (mp << 64);
    let mv = mp & m; // Bits to move.
    m = m ^ mv | (mv >> (1 << i)); // Compress m.
    let t = x & mv;
    x = x ^ t | (t >> (1 << i)); // Compress x.
    mk &= !mp;
  }
  x
}

fn mul_hi(a: u64, b: u64) -> u64 {
  ((a as u128 * b as u128) >> 64) as u64
}

fn scramble_u128(v: u128) -> u128 {
  const P1: u64 = 0xb8bf34e5043f1aca;
  const P2: u64 = 0x46602dbc434ec010;
  let x1 = mul_hi(v as u64, P1);
  let x2 = mul_hi((v >> 64) as u64, P2);
  (x1 as u128) + ((x2 as u128) << 64)
}

fn entropy_hash(bytes: &str) -> u64 {
  // const EXTRACT_MASK: u64 = 0x0101050c0d0c0c0d;
  // const EXTRACT_MASK: u128 = 0x00000000000000040405090d0d0d0c1c;
  // const EXTRACT_MASK: u128 = 0x00000000000000040005040d0c0d0c0d;
  const EXTRACT_MASK: u128 = 0x0000000000000000000000120a0e0f96;
  let v = unsafe { read_unaligned(bytes.as_ptr() as *const u128) };
  let v = mask_above(v, bytes.len()) as u128;
  let v = scramble_u128(v);
  compress(v, EXTRACT_MASK) as u64
}

fn run() -> BarseResult {
  let weather_stations = weather_stations("data/weather_stations.csv")?;
  const TABLE_BITS: u32 = 15;
  const CAP: usize = 1 << TABLE_BITS;

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
      |station| { str_hash(station.as_bytes()) },
      CAP
    )
  );

  println!(
    "New hash quality: {}",
    compute_hash_quality(&weather_stations, |station| { new_hash(station) }, CAP)
  );

  println!(
    "Entropy hash quality: {}",
    compute_hash_quality(&weather_stations, |station| { entropy_hash(station) }, CAP)
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
