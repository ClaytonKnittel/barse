use crate::temperature_reading::TemperatureReading;

#[derive(Debug, Clone, Copy)]
pub struct TemperatureSummary {
  pub min: TemperatureReading,
  pub max: TemperatureReading,
  pub total: i64,
  pub count: u32,
}

impl TemperatureSummary {
  pub fn initialize(&mut self) {
    self.min = TemperatureReading::new(i16::MAX);
    self.max = TemperatureReading::new(i16::MIN);
    debug_assert_eq!(self.total, 0);
    debug_assert_eq!(self.count, 0);
  }

  pub fn merge(&mut self, other: &Self) {
    self.min = self.min.min(other.min);
    self.max = self.max.max(other.max);
    self.total += other.total;
    self.count += other.count;
  }

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

  pub fn add_reading(&mut self, temp: TemperatureReading) {
    self.min = self.min.min(temp);
    self.max = self.max.max(temp);
    self.total += temp.reading() as i64;
    self.count += 1;
  }
}

impl Default for TemperatureSummary {
  fn default() -> Self {
    Self {
      min: TemperatureReading::new(i16::MAX),
      max: TemperatureReading::new(i16::MIN),
      total: 0,
      count: 0,
    }
  }
}

#[cfg(test)]
mod tests {
  use googletest::prelude::*;

  use crate::{temperature_reading::TemperatureReading, temperature_summary::TemperatureSummary};

  #[gtest]
  fn test_merge_default() {
    let mut s1 = TemperatureSummary {
      min: TemperatureReading::new(-15),
      max: TemperatureReading::new(20),
      total: 50,
      count: 5,
    };
    s1.merge(&TemperatureSummary::default());
    expect_that!(
      s1,
      pat!(TemperatureSummary {
        min: TemperatureReading::new(-15),
        max: TemperatureReading::new(20),
        total: 50,
        count: 5,
      })
    );
  }

  #[gtest]
  fn test_merge_default_lhs() {
    let mut s1 = TemperatureSummary::default();
    s1.merge(&TemperatureSummary {
      min: TemperatureReading::new(-15),
      max: TemperatureReading::new(20),
      total: 50,
      count: 5,
    });
    expect_that!(
      s1,
      pat!(TemperatureSummary {
        min: TemperatureReading::new(-15),
        max: TemperatureReading::new(20),
        total: 50,
        count: 5,
      })
    );
  }

  #[gtest]
  fn test_merge() {
    let mut s1 = TemperatureSummary {
      min: TemperatureReading::new(-10),
      max: TemperatureReading::new(25),
      total: 40,
      count: 4,
    };
    s1.merge(&TemperatureSummary {
      min: TemperatureReading::new(-15),
      max: TemperatureReading::new(20),
      total: 50,
      count: 5,
    });
    expect_that!(
      s1,
      pat!(TemperatureSummary {
        min: TemperatureReading::new(-15),
        max: TemperatureReading::new(25),
        total: 90,
        count: 9,
      })
    );
  }
}
