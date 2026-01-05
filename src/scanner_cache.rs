pub const BYTES_PER_BATCH: usize = 16;

pub fn read_next_from_buffer(buffer: &[u8]) -> (u64, u64) {
  let cache = unsafe { *(buffer.as_ptr() as *const u128) };
  let semicolon_mask = char_mask(cache, b';');
  let newline_mask = char_mask(cache, b'\n');
  (semicolon_mask, newline_mask)
}

fn compress_msb(val: u64) -> u64 {
  const MSB: u64 = 0x8080_8080_8080_8080;
  debug_assert!((val & !MSB) == 0);
  const COMPRESS_PRODUCT: u64 = 0x0002_0408_1020_4081;
  val.wrapping_mul(COMPRESS_PRODUCT) >> 56
}

fn find_zero_bytes(val: u128) -> u64 {
  const MSB: u128 = 0x8080_8080_8080_8080_8080_8080_8080_8080;
  let x = (val & !MSB) + !MSB;
  let y = !(x | val) & MSB;
  let lower_half = y as u64;
  let upper_half = (y >> 64) as u64;

  compress_msb(lower_half) + (compress_msb(upper_half) << 8)
}

fn char_mask(cache: u128, needle: u8) -> u64 {
  const LSB: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;
  let search_mask = LSB * needle as u128;
  let zero_mask = cache ^ search_mask;
  find_zero_bytes(zero_mask)
}

#[cfg(test)]
mod tests {
  use googletest::prelude::*;

  use crate::scanner_cache::find_zero_bytes;

  #[gtest]
  fn test_find_zero_bytes() {
    expect_eq!(
      find_zero_bytes(0x0101_0101_0101_0101_0101_0101_0101_0101),
      0x0000
    );
    expect_eq!(
      find_zero_bytes(0x0101_0101_0101_0101_0100_0101_0101_0101),
      0x0040
    );
    expect_eq!(
      find_zero_bytes(0x01ff_01ff_ffff_0101_ff00_01ff_0101_0101),
      0x0040
    );
    expect_eq!(
      find_zero_bytes(0x0100_0101_0000_0100_0101_0101_0000_0000),
      0x4d0f,
    );
  }
}
