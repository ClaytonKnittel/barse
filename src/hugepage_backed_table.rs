use std::marker::PhantomData;

use memmap2::{MmapMut, MmapOptions};

use crate::error::BarseResult;

pub const HUGEPAGE_SIZE: usize = 2 * 1024 * 1024;

pub trait InPlaceInitializable {
  fn initialize(&mut self);
}

pub struct HugepageBackedTable<T, const SIZE: usize> {
  elements: MmapMut,
  _phantom: PhantomData<T>,
}

impl<T: InPlaceInitializable, const SIZE: usize> HugepageBackedTable<T, SIZE> {
  pub fn new() -> BarseResult<Self> {
    let size = (SIZE * std::mem::size_of::<T>()).next_multiple_of(HUGEPAGE_SIZE);
    let elements = MmapOptions::new().len(size).map_anon()?;
    #[cfg(target_os = "linux")]
    elements.advise(memmap2::Advice::HugePage)?;

    let mut table = Self {
      elements,
      _phantom: PhantomData,
    };
    for i in 0..SIZE {
      table.entry_at_mut(i).initialize();
    }
    Ok(table)
  }
}

impl<T, const SIZE: usize> HugepageBackedTable<T, SIZE> {
  fn elements_ptr(&self) -> *const T {
    self.elements.as_ptr() as *const T
  }

  fn mut_elements_ptr(&mut self) -> *mut T {
    self.elements.as_mut_ptr() as *mut T
  }

  pub fn entry_at(&self, index: usize) -> &T {
    debug_assert!(index < SIZE);
    unsafe { &*self.elements_ptr().add(index) }
  }

  pub fn entry_at_mut(&mut self, index: usize) -> &mut T {
    debug_assert!(index < SIZE);
    unsafe { &mut *self.mut_elements_ptr().add(index) }
  }
}
