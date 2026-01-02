use std::slice;

use memmap2::MmapMut;

use crate::{
  error::BarseResult,
  inline_string_mt::InlineString,
  str_hash::str_hash,
  temperature_reading::TemperatureReading,
  temperature_summary::TemperatureSummary,
  util::{allocate_hugepages, HasIter, InPlaceInitializable},
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

  #[allow(clippy::mut_from_ref)]
  fn entries_from_parts(
    index: usize,
    elements: &[u8],
    n_threads: u32,
  ) -> (&InlineString, &mut [TemperatureSummary]) {
    let thread_local_array_offset = std::mem::size_of::<InlineString>();

    let entry_start_ptr = unsafe {
      elements
        .as_ptr()
        .byte_add(index * Self::element_size(n_threads))
    };
    let temp_summary_array_start_ptr = unsafe { entry_start_ptr.add(thread_local_array_offset) };
    unsafe {
      (
        &*(entry_start_ptr as *const InlineString),
        slice::from_raw_parts_mut(
          temp_summary_array_start_ptr as *mut TemperatureSummary,
          n_threads as usize,
        ),
      )
    }
  }

  fn initialize_elements(elements: &MmapMut, n_threads: u32) {
    for index in 0..SIZE {
      let (_, temp_summaries) = Self::entries_from_parts(index, elements, n_threads);
      for summary in temp_summaries {
        summary.initialize();
      }
    }
  }

  pub fn new(n_threads: u32) -> BarseResult<Self> {
    let table_size = Self::table_size(n_threads);
    let elements = allocate_hugepages(table_size)?;
    Self::initialize_elements(&elements, n_threads);
    Ok(Self {
      elements,
      n_threads,
    })
  }

  fn entries_at(&self, index: usize) -> (&InlineString, &mut [TemperatureSummary]) {
    Self::entries_from_parts(index, &self.elements, self.n_threads)
  }

  #[allow(clippy::mut_from_ref)]
  fn entry_at(&self, index: usize, thread_index: u32) -> (&InlineString, &mut TemperatureSummary) {
    debug_assert!(thread_index < self.n_threads);
    let (station, summaries) = self.entries_at(index);
    (station, unsafe {
      summaries.get_unchecked_mut(thread_index as usize)
    })
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
    (0..SIZE)
      .map(|index| self.entries_at(index))
      .filter(|(station, _)| station.initialized())
      .map(|(station, temp_summaries)| {
        (
          station.value_str(),
          temp_summaries
            .iter()
            .cloned()
            .reduce(|mut summary1, summary2| {
              summary1.merge(&summary2);
              summary1
            })
            .expect("Summary array should not be empty"),
        )
      })
  }
}
