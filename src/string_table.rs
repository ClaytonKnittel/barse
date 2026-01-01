use crate::{
  error::BarseResult, hugepage_backed_table::HugepageBackedTable, inline_string::InlineString,
};

pub struct StringTable<const SIZE: usize> {
  table: HugepageBackedTable<InlineString, SIZE>,
}

impl<const SIZE: usize> StringTable<SIZE> {
  pub fn new() -> BarseResult<Self> {
    Ok(Self {
      table: HugepageBackedTable::new()?,
    })
  }
}
