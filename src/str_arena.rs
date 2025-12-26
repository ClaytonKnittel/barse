use crate::inline_string::InlineString;

pub struct StringArena<const N: usize> {
  allocated: usize,
  strs: [InlineString; N],
}

impl<const N: usize> StringArena<N> {
  pub fn allocate(&mut self, contents: &str) -> &InlineString {
    todo!();
  }
}

impl<const N: usize> Default for StringArena<N> {
  fn default() -> Self {
    Self {
      allocated: 0,
      strs: [(); N].map(|_| InlineString::default()),
    }
  }
}
