use std::arch::x86_64::{
  __m256i, _mm256_and_si256, _mm256_loadu_si256, _mm256_testz_si256, _mm256_xor_si256,
};

use crate::{
  inline_string::InlineString,
  util::{unaligned_m256i_read_would_cross_page_boundary, unlikely},
};

const M256_BYTES: usize = 32;

fn cmp_str_slow(inline_str: &InlineString, other: &str) -> bool {
  inline_str.value_str() == other
}

fn foreign_str_unknown_bytes_mask(len: usize) -> __m256i {
  debug_assert!((0..=M256_BYTES).contains(&len));

  const MASK_BYTES: [u8; 63] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0,
  ];
  unsafe {
    _mm256_loadu_si256(MASK_BYTES.get_unchecked(M256_BYTES - len..).as_ptr() as *const __m256i)
  }
}

#[target_feature(enable = "avx2")]
fn cmp_si256(a: __m256i, b: __m256i) -> bool {
  let xor = _mm256_xor_si256(a, b);
  _mm256_testz_si256(xor, xor) != 0
}

#[target_feature(enable = "avx2")]
fn cmp_str_fast_avx(inline_str: &InlineString, other: &str) -> bool {
  let len = inline_str.len();

  let inline_str_val =
    unsafe { _mm256_loadu_si256(inline_str.value_str().as_ptr() as *const __m256i) };
  let mask = foreign_str_unknown_bytes_mask(len);
  let foreign_str_val = unsafe { _mm256_loadu_si256(other.as_ptr() as *const __m256i) };
  let other_str_val = _mm256_and_si256(foreign_str_val, mask);

  cmp_si256(inline_str_val, other_str_val)
}

pub fn inline_str_eq_foreign_str(inline_str: &InlineString, other: &str) -> bool {
  let len = inline_str.len();
  if unlikely(len != other.len()) {
    false
  } else if len > M256_BYTES
    || unlikely(unaligned_m256i_read_would_cross_page_boundary(
      other.as_ptr(),
    ))
  {
    cmp_str_slow(inline_str, other)
  } else {
    unsafe { cmp_str_fast_avx(inline_str, other) }
  }
}

#[cfg(test)]
mod tests {
  use googletest::prelude::*;

  use crate::{inline_string::InlineString, str_cmp_x86::inline_str_eq_foreign_str};

  #[gtest]
  fn test_cmp_eq() {
    expect_true!(inline_str_eq_foreign_str(
      &InlineString::new("test word"),
      "test word"
    ));
    expect_true!(inline_str_eq_foreign_str(&InlineString::new("a"), "a"));
    expect_true!(inline_str_eq_foreign_str(
      &InlineString::new("This sentence is 32 letters long"),
      "This sentence is 32 letters long"
    ));
    expect_true!(inline_str_eq_foreign_str(
      &InlineString::new("This sentence is more than 32 letters long"),
      "This sentence is more than 32 letters long"
    ));
  }

  #[gtest]
  fn test_cmp_ne() {
    expect_false!(inline_str_eq_foreign_str(
      &InlineString::new("test word"),
      "test word two"
    ));
    expect_false!(inline_str_eq_foreign_str(
      &InlineString::new("test word"),
      "test"
    ));
    expect_false!(inline_str_eq_foreign_str(
      &InlineString::new("test word"),
      "word test"
    ));
  }
}
