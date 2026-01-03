use std::{fs::File, io::Read};

fn main() {
  let mut buf = [0u8; 32 * 4096];
  let mut sum = 0;
  let mut total_len = 0;
  let mut file = File::open("measurements.txt").unwrap();
  while let Ok(len) = file.read(&mut buf) {
    if len == 0 {
      break;
    }
    total_len += len;
    sum += buf[..len].iter().cloned().map(|c| c as u64).sum::<u64>();
  }
  println!("Sum: {sum}");
  println!("Read {total_len} bytes");
}
