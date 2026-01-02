use std::marker::PhantomData;

use memmap2::MmapMut;

use crate::{
  error::BarseResult,
  util::{allocate_hugepages, InPlaceInitializable},
};

pub struct HugepageBackedTable<T, const SIZE: usize> {
  elements: MmapMut,
  _phantom: PhantomData<T>,
}

impl<T: InPlaceInitializable, const SIZE: usize> HugepageBackedTable<T, SIZE> {
  pub fn new() -> BarseResult<Self> {
    let elements = allocate_hugepages(SIZE * std::mem::size_of::<T>())?;

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
