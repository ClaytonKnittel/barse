use crate::{
  inline_string::InlineString,
  temperature_reading::TemperatureReading,
  temperature_summary::TemperatureSummary,
  util::{likely, InPlaceInitializable},
};

#[derive(Default, Clone)]
pub struct Entry {
  key: InlineString,
  temp_summary: TemperatureSummary,
}

impl Entry {
  fn initialize_key(&mut self, station: &str) {
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
      self.initialize_key(station);
      true
    } else {
      false
    }
  }

  pub fn is_default(&self) -> bool {
    self.key.is_default()
  }

  pub fn to_iter_pair(&self) -> (&str, TemperatureSummary) {
    (self.key.value_str(), self.temp_summary.clone())
  }
}

impl InPlaceInitializable for Entry {
  fn initialize(&mut self) {
    self.temp_summary.initialize();
  }
}
