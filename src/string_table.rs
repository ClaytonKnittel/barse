use memmap2::{MmapMut, MmapOptions};

use crate::{error::BarseResult, inline_string::InlineString, util::HUGEPAGE_SIZE};

pub struct StringTable<const SIZE: usize> {
  buckets: MmapMut,
}

impl<const SIZE: usize> StringTable<SIZE> {
  pub fn new() -> BarseResult<Self> {
    let size = (SIZE * std::mem::size_of::<InlineString>()).next_multiple_of(HUGEPAGE_SIZE);
    let buckets = MmapOptions::new().len(size).map_anon()?;
    buckets.advise(memmap2::Advice::HugePage)?;

    let mut s = Self { buckets };
    for i in 0..SIZE {
      s.entry_at_mut(i).initialize_to_default();
    }
    Ok(s)
  }
}
