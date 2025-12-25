use std::{fmt::Display, ptr::read_unaligned, str::FromStr};

use crate::error::BarseError;

const PARSE_TABLE_SHIFT: u32 = 13;
const PARSE_TABLE_SIZE: usize = 1 << PARSE_TABLE_SHIFT;
const PARSE_MAGIC: u64 = 0xb5a491adb02afa8c;

const fn int_val_to_str_encoding(val: i16) -> u64 {
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

  ascii_encoding
}

const fn parse_table_idx(float_string_encoding: u64) -> usize {
  (float_string_encoding.wrapping_mul(PARSE_MAGIC) >> (u64::BITS - PARSE_TABLE_SHIFT)) as usize
}

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

const PARSE_TABLE: [TemperatureReading; PARSE_TABLE_SIZE] = build_parse_table();

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TemperatureReading {
  reading: i16,
}

impl TemperatureReading {
  pub const fn new(reading: i16) -> Self {
    Self { reading }
  }

  pub const fn reading(&self) -> i16 {
    self.reading
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
    let val = unsafe { read_unaligned(s.as_ptr() as *const u64) };

    // Mask off all bytes starting from the newline:
    let mask = if val.to_ne_bytes()[3] == b'\n' {
      0x0000_0000_00ff_ffff
    } else if val.to_ne_bytes()[4] == b'\n' {
      0x0000_0000_ffff_ffff
    } else {
      debug_assert_eq!(val.to_ne_bytes()[5], b'\n');
      0x0000_00ff_ffff_ffff
    };
    let val = val & mask;

    unsafe { *PARSE_TABLE.get_unchecked(parse_table_idx(val)) }
  }
}

impl FromStr for TemperatureReading {
  type Err = BarseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    debug_assert!((3..=5).contains(&s.len()));
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

      let temp_reading = TemperatureReading::parse_float_manual(as_str);
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
