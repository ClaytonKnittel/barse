use crate::temperature_reading::TemperatureReading;

pub struct ScannerGeneric<'a> {
  buffer: &'a [u8],
  cache: u128,
  semicolon_mask: u16,
  newline_mask: u16,

  /// The offset of the previously-read newline character + 1, e.g. the
  /// starting point of the expected next weather station name.
  cur_offset: u16,
}

impl<'a> ScannerGeneric<'a> {
  /// The count of bytes that fit in a uint8x8_t
  const BYTES_PER_BUFFER: usize = std::mem::size_of::<u128>();

  pub fn new<'b: 'a>(buffer: &'b [u8]) -> Self {
    debug_assert!(buffer.len().is_multiple_of(Self::BYTES_PER_BUFFER));
    let (cache, semicolon_mask, newline_mask) = Self::read_next_from_buffer(buffer);
    Self {
      buffer,
      cache,
      semicolon_mask,
      newline_mask,
      cur_offset: 0,
    }
  }

  pub fn semicolon_mask(&self) -> u16 {
    self.semicolon_mask
  }

  pub fn newline_mask(&self) -> u16 {
    self.newline_mask
  }

  pub fn set_cur_offset(&mut self, offset: u16) {
    self.cur_offset = offset;
  }

  fn read_next_from_buffer(buffer: &[u8]) -> (u128, u16, u16) {
    let cache = unsafe { *(buffer.as_ptr() as *const u128) };
    let semicolon_mask = Self::char_mask(cache, b';');
    let newline_mask = Self::char_mask(cache, b'\n');
    (cache, semicolon_mask, newline_mask)
  }

  fn compress_msb(val: u64) -> u64 {
    const MSB: u64 = 0x8080_8080_8080_8080;
    debug_assert!((val & !MSB) == 0);
    const COMPRESS_PRODUCT: u64 = 0x0002_0408_1020_4081;
    val.wrapping_mul(COMPRESS_PRODUCT) >> 56
  }

  fn find_zero_bytes(val: u128) -> u16 {
    const MSB: u128 = 0x8080_8080_8080_8080_8080_8080_8080_8080;
    let x = (val & !MSB) + !MSB;
    let y = !(x | val) & MSB;
    let lower_half = y as u64;
    let upper_half = (y >> 64) as u64;

    (Self::compress_msb(lower_half) + (Self::compress_msb(upper_half) << 8)) as u16
  }

  fn char_mask(cache: u128, needle: u8) -> u16 {
    const LSB: u128 = 0x0101_0101_0101_0101_0101_0101_0101_0101;
    let search_mask = LSB * needle as u128;
    let zero_mask = cache ^ search_mask;
    Self::find_zero_bytes(zero_mask)
  }

  fn read_next_assuming_available(&mut self) {
    debug_assert!(self.buffer.len() > Self::BYTES_PER_BUFFER);
    self.buffer = &self.buffer[Self::BYTES_PER_BUFFER..];
    let (cache, semicolon_mask, newline_mask) = Self::read_next_from_buffer(self.buffer);
    self.cache = cache;
    self.semicolon_mask = semicolon_mask;
    self.newline_mask = newline_mask;
  }

  pub fn read_next(&mut self) -> bool {
    debug_assert!(!self.buffer.is_empty());
    if self.buffer.len() == Self::BYTES_PER_BUFFER {
      return false;
    }
    self.read_next_assuming_available();
    true
  }

  pub fn offset_to_ptr(&self, offset: u16) -> *const u8 {
    debug_assert!(offset <= Self::BYTES_PER_BUFFER as u16);
    unsafe { self.buffer.get_unchecked(offset as usize..) }.as_ptr()
  }

  pub fn cur_offset_to_ptr(&self) -> *const u8 {
    self.offset_to_ptr(self.cur_offset)
  }

  pub fn consume_next_semicolon(&mut self) -> u16 {
    debug_assert!(
      self.semicolon_mask != 0,
      "Expected non-empty semicolon mask after refreshing buffers in iteration"
    );
    let semicolon_offset = self.semicolon_mask.trailing_zeros();
    self.semicolon_mask &= self.semicolon_mask - 1;
    semicolon_offset as u16
  }

  pub fn find_next_station_name(&mut self) -> Option<&'a str> {
    None
  }

  pub fn find_next_temp_reading(&mut self) -> TemperatureReading {
    TemperatureReading::new(0)
  }
}

#[cfg(test)]
mod tests {
  use googletest::prelude::*;

  use crate::scanner_generic::ScannerGeneric;

  #[gtest]
  fn test_find_zero_bytes() {
    expect_eq!(
      ScannerGeneric::find_zero_bytes(0x0101_0101_0101_0101_0101_0101_0101_0101),
      0x0000
    );
    expect_eq!(
      ScannerGeneric::find_zero_bytes(0x0101_0101_0101_0101_0100_0101_0101_0101),
      0x0040
    );
    expect_eq!(
      ScannerGeneric::find_zero_bytes(0x01ff_01ff_ffff_0101_ff00_01ff_0101_0101),
      0x0040
    );
    expect_eq!(
      ScannerGeneric::find_zero_bytes(0x0100_0101_0000_0100_0101_0101_0000_0000),
      0x4d0f,
    );
  }
}
