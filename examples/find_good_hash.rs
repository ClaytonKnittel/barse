use core::f32;
use std::{
  collections::HashSet,
  fs::File,
  io::{BufRead, BufReader},
  process::ExitCode,
};

use barse::error::{BarseError, BarseResult};
use rand::{
  distr::{Distribution, Uniform},
  rng,
  seq::IteratorRandom,
  Rng, RngCore,
};

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

fn wymix(a: u64, b: u64) -> u64 {
  let r = (a as u128) * (b as u128);
  return (r as u64) ^ (r >> 64) as u64;
}

fn wyhash32(bytes: &[u8], seed: u64) -> u64 {
  let mut local_hash = [0u64; 4];
  for (dst, chunk) in local_hash
    .iter_mut()
    .zip(bytes.chunks(std::mem::size_of::<u64>()))
  {
    if chunk.len() == 8 {
      *dst = u64::from_ne_bytes(chunk.try_into().unwrap());
    } else {
      for (i, c) in chunk.iter().enumerate() {
        *dst += (*c as u64) << (8 * i);
      }
    }
  }

  let a = local_hash[0];
  let b = local_hash[1];
  let c = local_hash[2];
  let d = local_hash[3];

  let mut h = seed;
  h = wymix(h ^ a, 0x60bee2bee120fc15);
  h = wymix(h ^ b, 0xa3b195354a39b70d);
  h = wymix(h ^ c, 0x1b03738712fad5c9);
  wymix(h ^ d, 0x9e3779b97f4a7c15)
}

fn new_hash(bytes: &str, p: &[[u64; 2]; 4]) -> u64 {
  let mut local_hash = [0u64; 4];
  let mut add = p[0][1];
  for ((dst, chunk), &[mul, _]) in local_hash
    .iter_mut()
    .zip(bytes.as_bytes().chunks(std::mem::size_of::<u64>()))
    .zip(p)
  {
    if chunk.len() == 8 {
      *dst = u64::from_ne_bytes(chunk.try_into().unwrap());
    } else {
      for (i, c) in chunk.iter().enumerate() {
        *dst += (*c as u64) << (8 * i);
      }
    }
    // for i in 0..chunk.len() {
    //   let byte = (*dst >> (8 * i)) & 0xff;
    //   let byte = ((byte + (add >> (8 * i))) & 0xff) << (8 * i);
    //   let mask = 0xff << (8 * i);
    //   *dst = (*dst & !mask) + byte;
    // }
    *dst = scramble_u64(dst.wrapping_add(add), mul);
    add = *dst;
  }
  local_hash[0] ^ local_hash[1] ^ local_hash[2] ^ local_hash[3]
}

fn ncr(n: u32, r: u32) -> u64 {
  if n < r {
    0
  } else {
    (1..=n as u64).rev().take(r as usize).product::<u64>() / (1..=r as u64).product::<u64>()
  }
}

fn rand_u64_with_n_bits<R: Rng>(bits: u32, rng: &mut R) -> u64 {
  let distr = Uniform::new(0, ncr(64, bits)).unwrap();
  let mut sample = distr.sample(rng);

  let mut res = 0u64;
  let mut b = bits;
  for bit in (0..64).rev() {
    let ways = ncr(bit, b);
    if ways <= sample {
      sample -= ways;
      b -= 1;
      res += 1 << bit;

      if b == 0 {
        break;
      }
    }
  }
  debug_assert_eq!(sample, 0);
  debug_assert_eq!(b, 0);
  debug_assert_eq!(res.count_ones(), bits);

  res
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
  let mut rng = rng();

  const BITS: u32 = 10;
  let mut best_quality = f32::MAX;
  loop {
    // let p = [
    //   [rand_u64_with_n_bits(BITS, &mut rng), rng.next_u64()],
    //   [rand_u64_with_n_bits(BITS, &mut rng), rng.next_u64()],
    //   [rand_u64_with_n_bits(BITS, &mut rng), rng.next_u64()],
    //   [rand_u64_with_n_bits(BITS, &mut rng), rng.next_u64()],
    // ];
    let p = rng.next_u64();
    // let p = rand_u64_with_n_bits(BITS, &mut rng);
    // let p = [p, p, p, p];
    // let hash_fn = |bytes: &String| new_hash(bytes, &p);
    let hash_fn = |bytes: &String| wyhash32(bytes.as_bytes(), p);
    let quality = compute_hash_quality(&weather_stations, hash_fn, TABLE_SIZE);
    if quality < best_quality {
      best_quality = quality;
      println!("Quality {quality} for {p:08x?}");
      if quality < 1.04 {
        break;
      }
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
