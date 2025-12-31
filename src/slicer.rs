use std::{
  slice,
  sync::atomic::{AtomicUsize, Ordering},
};

use crate::scanner::{Scanner, BUFFER_OVERLAP};

const CHUNK_SIZE: usize = 2 * 1024 * 1024;

pub struct Slicer {
  buffer: &'static [u8],
  cur_offset: AtomicUsize,
}

impl Slicer {
  /// Safety:
  /// The caller must guarantee that the lifetime of `buffer` outlives
  /// `Scanner`.
  pub unsafe fn new(buffer: &[u8]) -> Self {
    Self {
      buffer: unsafe { slice::from_raw_parts(buffer.as_ptr(), buffer.len()) },
      cur_offset: AtomicUsize::new(0),
    }
  }

  pub fn next_slice(&self) -> Option<Scanner<'_>> {
    let offset = self.cur_offset.fetch_add(CHUNK_SIZE, Ordering::Relaxed);
    if offset >= self.buffer.len() {
      self.cur_offset.fetch_sub(CHUNK_SIZE, Ordering::Relaxed);
      None
    } else {
      let end = (offset + CHUNK_SIZE + BUFFER_OVERLAP).min(self.buffer.len());
      let slice = &self.buffer[offset..end];
      if offset == 0 {
        Some(Scanner::from_start(slice))
      } else {
        Some(Scanner::from_midpoint(slice))
      }
    }
  }
}
