use std::arch::x86_64::{
  __m128i, __m256i, _mm256_and_si256, _mm256_castsi256_si128, _mm256_load_si256,
  _mm256_loadu_si256, _mm_cvtsi128_si64, _mm_unpackhi_epi64, _mm_xor_si128,
};

use crate::{
  str_hash::StringHashResult,
  util::{unaligned_read_would_cross_page_boundary, unlikely, M256_BYTES},
};

pub fn read_str_to_m256_slow(s: &[u8]) -> __m256i {
  #[repr(align(32))]
  struct AlignedStorage([u8; M256_BYTES]);

  let mut storage = AlignedStorage([0; M256_BYTES]);
  for (dst, src) in storage.0.iter_mut().zip(s.iter()) {
    *dst = *src;
  }

  unsafe { _mm256_load_si256(storage.0.as_ptr() as *const __m256i) }
}

#[target_feature(enable = "avx2")]
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
fn mask_char_and_above(v: __m256i, len: usize) -> __m256i {
  debug_assert!(
    (2..=M256_BYTES).contains(&len),
    "len is outside the range 2..={M256_BYTES}: {len}"
  );
  let mask = foreign_str_unknown_bytes_mask(len);
  _mm256_and_si256(v, mask)
}

#[target_feature(enable = "sse2")]
fn compress_m128_to_u64(v: __m128i) -> u64 {
  let hi = _mm_unpackhi_epi64(v, v);
  let res = _mm_xor_si128(v, hi);
  _mm_cvtsi128_si64(res) as u64
}

fn scramble_u64(v: u64) -> u64 {
  const MAGIC: u64 = 0x20000400020001;
  v.wrapping_mul(MAGIC) >> 48
}

#[target_feature(enable = "avx2")]
pub fn str_hash_fast(bytes: &[u8]) -> StringHashResult {
  let ptr = bytes.as_ptr();
  let raw_str_val = if unlikely(unaligned_read_would_cross_page_boundary::<__m256i>(ptr)) {
    read_str_to_m256_slow(bytes)
  } else {
    unsafe { _mm256_loadu_si256(ptr as *const __m256i) }
  };

  let len = bytes.len().min(M256_BYTES);
  let str_val = mask_char_and_above(raw_str_val, len);
  let str_val_m128 = _mm256_castsi256_si128(str_val);
  let v = compress_m128_to_u64(str_val_m128);
  let v = scramble_u64(v);

  StringHashResult {
    hash: v,
    masked_str_bytes: str_val,
  }
}
