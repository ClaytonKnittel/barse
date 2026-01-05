use std::marker::PhantomData;

use memmap2::{MmapMut, MmapOptions};

use crate::error::BarseResult;

pub const HUGEPAGE_SIZE: usize = 2 * 1024 * 1024;

/// A trait for objects which can be initialized from zero-initialized memory.
/// Implementers may assume `self` references zero-initialized memory.
pub trait InPlaceInitializable {
  /// Initialize `self` from zero-initialized bytes spanning
  /// `std::mem::size_of::<Self>()` bytes.
  fn initialize(&mut self);
}

/// An array of `T`s with constant `SIZE` elements allocated from `mmap`,
/// backed by hugepages on systems that support it.
pub struct HugepageBackedTable<T, const SIZE: usize> {
  /// The mmapped region of `SIZE` elements of type `T`.
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
  /// Returns a pointer to the start of the table.
  fn elements_ptr(&self) -> *const T {
    self.elements.as_ptr() as *const T
  }

  /// Returns a mut pointer to the start of the table.
  fn mut_elements_ptr(&mut self) -> *mut T {
    self.elements.as_mut_ptr() as *mut T
  }

  /// Returns a reference to the element at position `index` in the table.
  pub fn entry_at(&self, index: usize) -> &T {
    debug_assert!(index < SIZE);
    unsafe { &*self.elements_ptr().add(index) }
  }

  /// Returns a mutable reference to the element at position `index` in the table.
  pub fn entry_at_mut(&mut self, index: usize) -> &mut T {
    debug_assert!(index < SIZE);
    unsafe { &mut *self.mut_elements_ptr().add(index) }
  }
}
