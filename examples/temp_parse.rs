use std::{
  process::ExitCode,
  sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
  },
};

use barse::error::{BarseError, BarseResult};
use itertools::Itertools;
use rand::{rng, RngCore};

struct BitVec {
  vec: Vec<u64>,
}

impl BitVec {
  fn new(capacity: usize) -> Self {
    Self {
      vec: vec![0; capacity.div_ceil(u64::BITS as usize)],
    }
  }

  fn set(&mut self, idx: usize) -> bool {
    let i = idx / u64::BITS as usize;
    let j = idx % u64::BITS as usize;
    let mask = 1 << j;
    let prev_mask = self.vec[i];
    self.vec[i] = prev_mask | mask;
    (prev_mask & mask) != 0
  }
}

fn generate_input() -> impl Iterator<Item = u64> {
  (-999..=999).map(|n: i32| {
    let sign = if n < 0 { "-" } else { "" };
    let tens = n.abs() / 10;
    let ones = n.abs() % 10;
    let s = format!("{sign}{tens}.{ones}");
    s.as_bytes()
      .iter()
      .enumerate()
      .map(|(i, b)| (*b as u64) << (i * 8))
      .sum()
  })
}

fn all_unique_with_bits(universe: &[u64], magic: u64, bits: u32) -> bool {
  let mut bitv = BitVec::new(1 << bits);

  universe.iter().all(|num| {
    let hash = num.wrapping_mul(magic) >> (u64::BITS - bits);
    debug_assert!(hash < (1 << bits));
    !bitv.set(hash as usize)
  })
}

fn run() -> BarseResult {
  let all_values = Arc::new(generate_input().collect_vec());

  let fewest_bits = Arc::new(AtomicU32::new(20));
  let threads = (0..10)
    .map(|_| {
      let all_values = all_values.clone();
      let fewest_bits = fewest_bits.clone();
      std::thread::spawn(move || {
        let mut rng = rng();

        let mut prev_fewest_bits = fewest_bits.load(Ordering::SeqCst);
        while (1 << prev_fewest_bits) > all_values.len() {
          let magic = rng.next_u64();

          let mut attempt = prev_fewest_bits - 1;

          while all_unique_with_bits(&all_values, magic, attempt) {
            println!("Magic {magic:016x} unique with size {}", 1 << attempt);

            while prev_fewest_bits > attempt {
              match fewest_bits.compare_exchange_weak(
                prev_fewest_bits,
                attempt,
                Ordering::SeqCst,
                Ordering::SeqCst,
              ) {
                Ok(_) => {
                  prev_fewest_bits = attempt;
                  break;
                }
                Err(new_fewest_bits) => prev_fewest_bits = new_fewest_bits,
              }
            }

            attempt = prev_fewest_bits - 1;
          }

          prev_fewest_bits = fewest_bits.load(Ordering::SeqCst);
        }
      })
    })
    .collect_vec();

  for thread in threads {
    thread
      .join()
      .map_err(|err| BarseError::new(format!("Failed to join thread: {err:?}")))?;
  }

  Ok(())
}

fn main() -> ExitCode {
  #[cfg(feature = "profiled")]
  let guard = pprof::ProfilerGuardBuilder::default()
    .frequency(1000)
    .blocklist(&["libc", "libgcc", "pthread", "vdso"])
    .build()
    .unwrap();

  let res = run();

  #[cfg(feature = "profiled")]
  if let Ok(report) = guard.report().build() {
    let file = std::fs::File::create("temp_parse_magic_search.svg").unwrap();
    report.flamegraph(file).unwrap();
  };

  if let Err(err) = res {
    println!("{err}");
    ExitCode::FAILURE
  } else {
    ExitCode::SUCCESS
  }
}
