use std::{fmt::Display, ptr::read_unaligned, str::FromStr};

use crate::error::BarseError;

// Min and max possible temperature readings per the spec (-99.9 degrees to
// 99.9 degrees).
const MIN_TEMP: i16 = -999;
const MAX_TEMP: i16 = 999;

/// The log2 size of the temperature parse table, i.e. the number of bits
/// necessary for there to be no collisions in the perfect hashing scheme.
const PARSE_TABLE_SHIFT: u32 = 13;
/// The number of entries in the temperature reading parse table.
const PARSE_TABLE_SIZE: usize = 1 << PARSE_TABLE_SHIFT;
/// A magic number found from `examples/temp_parse.rs` which, under
/// multiplication, maps each possible temperature string u64 encoding to a u64
/// value with unique high 13 bits.
const PARSE_MAGIC: u64 = 0xd6df3436fe286720;

/// The fewest number of bytes possible in a valid temperature string encoding
/// (e.g. X.X).
pub const MIN_TEMP_READING_LEN: usize = 3;
/// The highest number of bytes possible in a valid temperature string encoding
/// (e.g. -XX.X).
pub const MAX_TEMP_READING_LEN: usize = 5;

/// Converts an integer encoding of a temperature reading to its string
/// representation in the file.
const fn int_val_to_str_encoding(val: i16) -> u64 {
  debug_assert!(val >= MIN_TEMP);
  debug_assert!(val <= MAX_TEMP);
  let mut ascii_encoding = 0;
  let mut ascii_idx = 0;

  const fn write_char(ascii_encoding: &mut u64, ascii_idx: &mut u32, c: u8) {
    debug_assert!(*ascii_idx < 8);
    *ascii_encoding += (c as u64) << (*ascii_idx * 8);
    *ascii_idx += 1;
  }

  if val < 0 {
    write_char(&mut ascii_encoding, &mut ascii_idx, b'-');
  }

  let pos_val = val.abs();
  if pos_val >= 100 {
    write_char(
      &mut ascii_encoding,
      &mut ascii_idx,
      (pos_val / 100) as u8 + b'0',
    );
  }
  write_char(
    &mut ascii_encoding,
    &mut ascii_idx,
    (pos_val / 10 % 10) as u8 + b'0',
  );
  write_char(&mut ascii_encoding, &mut ascii_idx, b'.');
  write_char(
    &mut ascii_encoding,
    &mut ascii_idx,
    (pos_val % 10) as u8 + b'0',
  );
  if ascii_idx < MAX_TEMP_READING_LEN as u32 {
    write_char(&mut ascii_encoding, &mut ascii_idx, b'\n');
  }

  ascii_encoding
}

/// Translates a temperature string value held in a u64 in little endian order
/// to the index in the parse table.
const fn parse_table_idx(float_string_encoding: u64) -> usize {
  (float_string_encoding.wrapping_mul(PARSE_MAGIC) >> (u64::BITS - PARSE_TABLE_SHIFT)) as usize
}

/// Builds a parse table which maps string encodings of temperatures to their
/// integer representation using multiply-rightshift perfect hashing.
const fn build_parse_table() -> [TemperatureReading; PARSE_TABLE_SIZE] {
  let mut table = [TemperatureReading::new(0); PARSE_TABLE_SIZE];
  let mut val = -999i16;
  while val <= 999 {
    let ascii_encoding = int_val_to_str_encoding(val);
    let idx = parse_table_idx(ascii_encoding);
    debug_assert!(table[idx].reading() == 0);
    table[idx] = TemperatureReading::new(val);

    val += 1;
  }
  table
}

/// Precomputed table mapping string encodings of temperatures to their integer
/// representations.
const PARSE_TABLE: [TemperatureReading; PARSE_TABLE_SIZE] = build_parse_table();

/// Represents a temperature reading from the input file, ranging from -99.9 to
/// 99.9 (2001 possible values).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TemperatureReading {
  /// Fixed-point representation of the temperature reading, i.e. 10 *
  /// temperature reading.
  reading: i16,
}

impl TemperatureReading {
  pub const fn new(reading: i16) -> Self {
    Self { reading }
  }

  pub fn from_raw_ptr(str_ptr: *const u8) -> Self {
    let encoding = unsafe { read_unaligned(str_ptr as *const u64) }.to_le();
    Self::u64_encoding_to_self(encoding)
  }

  pub const fn reading(&self) -> i16 {
    self.reading
  }

  /// Converts the string encoding of a temperature reading read directly from
  /// the file in little-endian order to a TemperatureReading. `encoding` is
  /// expected to contain a newline character (`b'\n'`) at some byte index
  /// 3 - 5, since temperature readings are always proceeded by a newline
  /// character.
  fn u64_encoding_to_self(encoding: u64) -> Self {
    let mask = if encoding.to_le_bytes()[3] == b'\n' {
      // If the character at index 3 in `encoding` is a newline, mask off byte
      // indices 4 - 7 since those may contain arbitrary values from the next
      // line of the file. I have chosen to keep the newline character in
      // `encoding` for consistency with the other branch.
      0x0000_0000_ffff_ffff
    } else {
      // Otherwise, either byte index 4 or 5 contains a newline character.
      debug_assert!(
        encoding.to_le_bytes()[4] == b'\n' || encoding.to_le_bytes()[5] == b'\n',
        "Encoding: {encoding:016x}, newline = {:02x}",
        b'\n'
      );
      // In this case, we unconditionally keep the first 5 characters to avoid
      // a branch. For 4-byte temperature readings, this will include the
      // trailing newline, and for 5-byte readings it will exactly encompass
      // all 5 bytes.
      0x0000_00ff_ffff_ffff
    };
    // `val` is a unique integer value for each possible temperature reading.
    let val = encoding & mask;

    // Look up the parsed temperature reading from a precomputed lookup table.
    unsafe { *PARSE_TABLE.get_unchecked(parse_table_idx(val)) }
  }

  #[cfg(test)]
  fn parse_float_manual(s: &str) -> Self {
    let tens: i16 = unsafe { s[..s.len() - 2].parse().unwrap_unchecked() };
    let mut ones = (s.as_bytes()[s.len() - 1] - b'0') as i16;
    if s.as_bytes()[0] == b'-' {
      ones = -ones;
    }
    Self {
      reading: tens * 10 + ones,
    }
  }

  fn parse_float_magic(s: &str) -> Self {
    Self::from_raw_ptr(s.as_ptr())
  }
}

impl FromStr for TemperatureReading {
  type Err = BarseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    debug_assert!((MIN_TEMP_READING_LEN..=MAX_TEMP_READING_LEN).contains(&s.len()));
    debug_assert_eq!(s.as_bytes()[s.len() - 2], b'.');
    Ok(Self::parse_float_magic(s))
  }
}

impl Display for TemperatureReading {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let neg = if self.reading < 0 { "-" } else { "" };
    let tens = self.reading.abs() / 10;
    let ones = self.reading.abs() % 10;
    write!(f, "{neg}{tens}.{ones}")
  }
}

#[cfg(test)]
mod tests {
  use std::slice;

  use itertools::Itertools;

  use crate::temperature_reading::{
    int_val_to_str_encoding, parse_table_idx, TemperatureReading, PARSE_TABLE,
  };

  fn int_val_to_str(val: i16) -> String {
    let sign = if val < 0 { "-" } else { "" };
    let tens = val.abs() / 10;
    let ones = val.abs() % 10;
    format!("{sign}{tens}.{ones}")
  }

  #[test]
  fn test_int_val_to_str_encoding() {
    for val in -999..=999 {
      let encoding = int_val_to_str_encoding(val);
      let first_zero_byte = encoding
        .to_ne_bytes()
        .iter()
        .find_position(|b| **b == 0)
        .unwrap()
        .0;
      let bytes = encoding.to_ne_bytes();
      let as_str =
        str::from_utf8(unsafe { slice::from_raw_parts(bytes.as_ptr(), first_zero_byte) }).unwrap();

      let temp_reading =
        TemperatureReading::parse_float_manual(as_str.strip_suffix('\n').unwrap_or(as_str));
      assert_eq!(temp_reading.reading(), val);
    }
  }

  #[test]
  fn test_parse_table() {
    for val in -999..=999 {
      let table_idx = parse_table_idx(int_val_to_str_encoding(val));
      assert_eq!(PARSE_TABLE[table_idx].reading(), val);
    }
  }

  #[test]
  fn test_parse() {
    for val in -999..=999 {
      let s = format!("{}\nab\n", int_val_to_str(val));
      let to_parse = s.strip_suffix("\nab\n").unwrap();
      println!("Parsing {to_parse}");
      assert_eq!(
        TemperatureReading::parse_float_magic(to_parse),
        TemperatureReading::parse_float_manual(to_parse),
        "Parsing {to_parse}"
      );
    }
  }
}
