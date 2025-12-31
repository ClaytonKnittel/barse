use crate::{inline_string::InlineString, temperature_reading::TemperatureReading, util::likely};

#[cfg(feature = "multithreaded")]
use crate::temperature_summary_mt::AtomicTemperatureSummary;

#[derive(Default)]
#[cfg_attr(test, derive(Clone))]
pub struct Entry {
  key: InlineString,
  #[cfg(feature = "multithreaded")]
  temp_summary: AtomicTemperatureSummary,
  #[cfg(not(feature = "multithreaded"))]
  temp_summary: TemperatureSummary,
}

impl Entry {
  pub fn initialize_to_default(&mut self) {
    self.temp_summary.initialize();
  }

  pub fn is_default(&self) -> bool {
    self.key.is_default()
  }

  pub fn to_iter_pair(&self) -> (&str, &TemperatureSummary) {
    (self.key.value_str(), &self.temp_summary)
  }
}

#[cfg(feature = "multithreaded")]
impl Entry {
  fn initialize(&self, station: &str) {
    self.key.initialize(station);
  }

  pub fn add_reading(&self, reading: TemperatureReading) {
    debug_assert!(!self.is_default());
    self.temp_summary.add_reading(reading);
  }

  pub fn matches_key_or_initialize(&self, station: &str) -> bool {
    if likely(self.key.eq_foreign_str(station)) {
      true
    } else if self.is_default() {
      self.initialize(station);
      true
    } else {
      false
    }
  }
}

#[cfg(not(feature = "multithreaded"))]
impl Entry {
  fn initialize(&mut self, station: &str) {
    self.key.initialize(station);
  }

  pub fn add_reading(&mut self, reading: TemperatureReading) {
    debug_assert!(!self.is_default());
    self.temp_summary.add_reading(reading);
  }

  pub fn matches_key_or_initialize(&mut self, station: &str) -> bool {
    if likely(self.key.eq_foreign_str(station)) {
      true
    } else if self.is_default() {
      self.initialize(station);
      true
    } else {
      false
    }
  }
}
