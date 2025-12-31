use std::sync::atomic::{AtomicI16, AtomicI64, AtomicU32, Ordering};

use crate::{temperature_reading::TemperatureReading, temperature_summary::TemperatureSummary};

#[derive(Debug)]
pub struct AtomicTemperatureSummary {
  pub min: AtomicI16,
  pub max: AtomicI16,
  pub total: AtomicI64,
  pub count: AtomicU32,
}

impl AtomicTemperatureSummary {
  pub fn initialize(&mut self) {
    self.min = AtomicI16::new(i16::MAX);
    self.max = AtomicI16::new(i16::MIN);
    debug_assert_eq!(self.total.load(Ordering::Relaxed), 0);
    debug_assert_eq!(self.count.load(Ordering::Relaxed), 0);
  }

  pub fn min(&self) -> TemperatureReading {
    TemperatureReading::new(self.min.load(Ordering::Relaxed))
  }

  pub fn max(&self) -> TemperatureReading {
    TemperatureReading::new(self.max.load(Ordering::Relaxed))
  }

  pub fn avg(&self) -> TemperatureReading {
    let total = self.total.load(Ordering::Relaxed);
    let count = self.count.load(Ordering::Relaxed);
    let rounding_offset = count as i64 / 2;
    let avg = (total + rounding_offset).div_euclid(count as i64);
    debug_assert!((i16::MIN as i64..=i16::MAX as i64).contains(&avg));
    TemperatureReading::new(avg as i16)
  }

  fn apply_to_i16<F>(val: &AtomicI16, mut f: F)
  where
    F: FnMut(i16) -> i16,
  {
    let mut old_v = val.load(Ordering::Relaxed);
    loop {
      let new_v = f(old_v);
      match val.compare_exchange_weak(old_v, new_v, Ordering::Relaxed, Ordering::Relaxed) {
        Ok(_) => break,
        Err(cur_v) => old_v = cur_v,
      }
    }
  }

  pub fn add_reading(&self, temp: TemperatureReading) {
    let temp = temp.reading();
    Self::apply_to_i16(&self.min, |min_temp| temp.min(min_temp));
    Self::apply_to_i16(&self.max, |max_temp| temp.max(max_temp));
    self.total.fetch_add(temp as i64, Ordering::Relaxed);
    self.count.fetch_add(1, Ordering::Relaxed);
  }
}

impl Default for AtomicTemperatureSummary {
  fn default() -> Self {
    Self {
      min: i16::MAX.into(),
      max: i16::MIN.into(),
      total: 0.into(),
      count: 0.into(),
    }
  }
}

impl From<AtomicTemperatureSummary> for TemperatureSummary {
  fn from(value: AtomicTemperatureSummary) -> Self {
    TemperatureSummary {
      min: value.min(),
      max: value.max(),
      total: value.total.load(Ordering::Relaxed),
      count: value.count.load(Ordering::Relaxed),
    }
  }
}
