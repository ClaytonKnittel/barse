use std::{
  alloc::{alloc, dealloc, Layout},
  slice,
};

use brc::build_input::{get_weather_stations, output_lines};
use rand::{rngs::StdRng, SeedableRng};

use crate::{error::BarseResult, temperature_reading::TemperatureReading};

const ALIGNMENT: usize = 32;

#[repr(align(32))]
pub struct AlignedBuffer<const N: usize> {
  pub buffer: [u8; N],
}

pub struct AlignedInput {
  bytes: *mut u8,
  len: usize,
}
impl AlignedInput {
  pub fn new(src: &str) -> Self {
    let len = src.len().next_multiple_of(ALIGNMENT);
    let layout = Layout::from_size_align(len, ALIGNMENT).unwrap();
    let bytes = unsafe { alloc(layout) };
    unsafe {
      libc::memset(bytes as *mut libc::c_void, 0, len);
      bytes.copy_from(src.as_bytes().as_ptr(), src.len());
    }
    Self {
      bytes,
      len: src.len(),
    }
  }

  pub fn exact_slice(&self) -> &[u8] {
    unsafe { slice::from_raw_parts(self.bytes, self.len) }
  }

  pub fn padded_slice(&self) -> &[u8] {
    unsafe { slice::from_raw_parts(self.bytes, self.len.next_multiple_of(ALIGNMENT)) }
  }
}
impl Drop for AlignedInput {
  fn drop(&mut self) {
    let layout = Layout::from_size_align(self.len, ALIGNMENT).unwrap();
    unsafe {
      dealloc(self.bytes, layout);
    }
  }
}

pub fn random_input_file(
  seed: u64,
  records: u64,
  unique_stations: u32,
) -> BarseResult<AlignedInput> {
  const WEATHER_STATIONS_PATH: &str = "data/weather_stations.csv";

  let mut rng = StdRng::seed_from_u64(seed);
  let stations = get_weather_stations(WEATHER_STATIONS_PATH).unwrap();

  Ok(AlignedInput::new(
    &output_lines(&stations, records, unique_stations, &mut rng)?
      .collect::<std::result::Result<Vec<_>, _>>()?
      .join(""),
  ))
}

pub fn simple_scanner_iter(buffer: &[u8]) -> impl Iterator<Item = (&str, TemperatureReading)> {
  str::from_utf8(buffer)
    .unwrap()
    .split('\n')
    .filter(|line| !line.is_empty() && !line.starts_with(0 as char))
    .map(|line| {
      let (station, temp) = line.split_once(';').unwrap();
      let temp = (temp.parse::<f32>().unwrap() * 10.).round() as i16;
      (station, TemperatureReading::new(temp))
    })
}
