use std::{
  slice,
  sync::atomic::{AtomicU64, Ordering},
};

use crate::{temperature_reading::TemperatureReading, util::InPlaceInitializable};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TemperatureSummary {
  pub min: TemperatureReading,
  pub max: TemperatureReading,
  pub count: u32,
  pub total: i64,
}

impl TemperatureSummary {
  const SIZE_OF_U64_SPAN: usize = std::mem::size_of::<Self>() / std::mem::size_of::<AtomicU64>();

  pub fn min(&self) -> TemperatureReading {
    self.min
  }

  pub fn max(&self) -> TemperatureReading {
    self.max
  }

  pub fn avg(&self) -> TemperatureReading {
    let rounding_offset = self.count as i64 / 2;
    let avg = (self.total + rounding_offset).div_euclid(self.count as i64);
    debug_assert!((i16::MIN as i64..=i16::MAX as i64).contains(&avg));
    TemperatureReading::new(avg as i16)
  }

  pub fn add_reading(&self, temp: TemperatureReading) {
    let atomic_u64_slice = unsafe {
      slice::from_raw_parts(
        self as *const Self as *const AtomicU64,
        Self::SIZE_OF_U64_SPAN,
      )
    };

    let (mut lo, mut hi) = (
      atomic_u64_slice[0].load(Ordering::Relaxed),
      atomic_u64_slice[1].load(Ordering::Relaxed),
    );
    loop {
      let mut tmp = [lo, hi];
      let mut local_test = unsafe { *(tmp.as_ptr() as *const Self) };
      local_test.min = local_test.min.min(temp);
      local_test.max = local_test.max.max(temp);
      local_test.count += 1;
      local_test.total += temp.reading() as i64;

      *unsafe { &mut *(tmp.as_mut_ptr() as *mut Self) } = local_test;

      let new_vals = (
        atomic_u64_slice[0].swap(tmp[0], Ordering::Relaxed),
        atomic_u64_slice[1].swap(tmp[1], Ordering::Relaxed),
      );
      if new_vals.0 == lo && new_vals.1 == hi {
        break;
      }
      break;
      (lo, hi) = (tmp[0], tmp[1]);
    }
  }
}

impl InPlaceInitializable for TemperatureSummary {
  fn initialize(&mut self) {
    self.min = TemperatureReading::new(i16::MAX);
    self.max = TemperatureReading::new(i16::MIN);
    debug_assert_eq!(self.count, 0);
    debug_assert_eq!(self.total, 0);
  }
}

impl Default for TemperatureSummary {
  fn default() -> Self {
    Self {
      min: TemperatureReading::new(i16::MAX),
      max: TemperatureReading::new(i16::MIN),
      count: 0,
      total: 0,
    }
  }
}
