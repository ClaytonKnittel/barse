use std::arch::x86_64::{__m256i, _mm256_loadu_si256, _mm256_testz_si256, _mm256_xor_si256};

use crate::{
  inline_string::InlineString,
  util::{unlikely, M256_BYTES},
};

#[cold]
fn cmp_str_slow(inline_str: &InlineString, other: &str) -> bool {
  inline_str.value_str() == other
}

#[target_feature(enable = "avx2")]
fn cmp_si256(a: __m256i, b: __m256i) -> bool {
  let xor = _mm256_xor_si256(a, b);
  _mm256_testz_si256(xor, xor) != 0
}

#[target_feature(enable = "avx2")]
fn cmp_str_fast_avx(inline_str: &InlineString, other_str_val: __m256i) -> bool {
  let inline_str_val =
    unsafe { _mm256_loadu_si256(inline_str.value_str().as_ptr() as *const __m256i) };
  cmp_si256(inline_str_val, other_str_val)
}

pub fn inline_str_eq_foreign_str(
  inline_str: &InlineString,
  other_str_val: __m256i,
  other: &str,
) -> bool {
  let len = inline_str.len();
  if unlikely(len != other.len()) {
    false
  } else if len > M256_BYTES {
    cmp_str_slow(inline_str, other)
  } else {
    unsafe { cmp_str_fast_avx(inline_str, other_str_val) }
  }
}

#[cfg(test)]
mod tests {
  use googletest::prelude::*;

  use crate::{
    inline_string::InlineString, str_cmp_x86::inline_str_eq_foreign_str,
    str_hash_x86::read_str_to_m256_slow,
  };

  fn inline_str_eq_foreign_str_for_test(inline_str: &InlineString, word: &str) -> bool {
    inline_str_eq_foreign_str(inline_str, read_str_to_m256_slow(word.as_bytes()), word)
  }

  #[gtest]
  fn test_cmp_eq() {
    expect_true!(inline_str_eq_foreign_str_for_test(
      &InlineString::new("test word"),
      "test word"
    ));
    expect_true!(inline_str_eq_foreign_str_for_test(
      &InlineString::new("a"),
      "a"
    ));
    expect_true!(inline_str_eq_foreign_str_for_test(
      &InlineString::new("This sentence is 32 letters long"),
      "This sentence is 32 letters long"
    ));
    expect_true!(inline_str_eq_foreign_str_for_test(
      &InlineString::new("This sentence is more than 32 letters long"),
      "This sentence is more than 32 letters long"
    ));
  }

  #[gtest]
  fn test_cmp_ne() {
    expect_false!(inline_str_eq_foreign_str_for_test(
      &InlineString::new("test word"),
      "test word two"
    ));
    expect_false!(inline_str_eq_foreign_str_for_test(
      &InlineString::new("test word"),
      "test"
    ));
    expect_false!(inline_str_eq_foreign_str_for_test(
      &InlineString::new("test word"),
      "word test"
    ));
  }
}
