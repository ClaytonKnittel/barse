use memmap2::MmapMut;

use crate::{
  error::BarseResult, hugepage_backed_table::allocate_hugepages, inline_string_mt::InlineString,
  str_hash::str_hash, temperature_reading::TemperatureReading,
  temperature_summary::TemperatureSummary, util::HasIter,
};

pub struct SharedTable<const SIZE: usize> {
  elements: MmapMut,
  n_threads: u32,
}

impl<const SIZE: usize> SharedTable<SIZE> {
  fn element_size(n_threads: u32) -> usize {
    std::mem::size_of::<InlineString>()
      + n_threads as usize * std::mem::size_of::<TemperatureSummary>()
  }

  fn table_size(n_threads: u32) -> usize {
    Self::element_size(n_threads) * SIZE
  }

  pub fn new(n_threads: u32) -> BarseResult<Self> {
    let table_size = Self::table_size(n_threads);
    let elements = allocate_hugepages(table_size)?;
    Ok(Self {
      elements,
      n_threads,
    })
  }

  fn elements_ptr(&self) -> *mut u8 {
    self.elements.as_ptr() as *mut u8
  }

  fn entry_at(&self, index: usize, thread_index: u32) -> (&InlineString, &mut TemperatureSummary) {
    debug_assert!(thread_index < self.n_threads);
    let thread_local_offset = std::mem::size_of::<InlineString>()
      + thread_index as usize * std::mem::size_of::<TemperatureSummary>();

    let entry_start_ptr = unsafe {
      self
        .elements_ptr()
        .byte_add(index * Self::element_size(self.n_threads))
    };
    let temp_summary_start_ptr = unsafe { entry_start_ptr.add(thread_local_offset) };
    unsafe {
      (
        &*(entry_start_ptr as *const InlineString),
        &mut *(temp_summary_start_ptr as *mut TemperatureSummary),
      )
    }
  }

  fn station_hash(&self, station: &str) -> u64 {
    str_hash(station.as_bytes())
  }

  fn station_index(&self, station: &str) -> usize {
    self.station_hash(station) as usize % SIZE
  }

  fn scan_for_entry(
    &self,
    station: &str,
    start_idx: usize,
    thread_index: u32,
  ) -> &mut TemperatureSummary {
    (1..SIZE)
      .map(|i| (start_idx + i) % SIZE)
      .map(|idx| self.entry_at(idx, thread_index))
      .find(|(station_slot, _)| station_slot.eq_or_initialize(station))
      .map(|(_, temp_summary)| temp_summary)
      .expect("No empty bucket found, table is full")
  }

  fn find_entry(&self, station: &str, thread_index: u32) -> &mut TemperatureSummary {
    let idx = self.station_index(station);
    let (station_slot, temp_summary) = self.entry_at(idx, thread_index);
    if station_slot.eq_or_initialize(station) {
      temp_summary
    } else {
      self.scan_for_entry(station, idx, thread_index)
    }
  }

  pub fn add_reading(&self, station: &str, temp: TemperatureReading, thread_index: u32) {
    let temp_summary = self.find_entry(station, thread_index);
    temp_summary.add_reading(temp);
  }
}

impl<'a, const SIZE: usize> HasIter<'a> for SharedTable<SIZE> {
  type Item = (&'a str, TemperatureSummary);

  fn iter(&'a self) -> impl Iterator<Item = Self::Item> {
    std::iter::empty()
  }
}
