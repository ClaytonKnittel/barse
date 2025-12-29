use std::arch::x86_64::{
  __m128i, _mm_and_si128, _mm_cvtsi128_si64, _mm_load_si128, _mm_loadu_si128, _mm_unpackhi_epi64,
  _mm_xor_si128,
};

use crate::util::{unaligned_read_would_cross_page_boundary, unlikely};

fn read_str_to_m128_slow(s: &[u8]) -> __m128i {
  #[repr(align(16))]
  struct AlignedStorage([u8; 16]);

  let mut storage = AlignedStorage([0; 16]);
  for (dst, src) in storage.0.iter_mut().zip(s.iter()) {
    *dst = *src;
  }

  unsafe { _mm_load_si128(storage.0.as_ptr() as *const __m128i) }
}

#[target_feature(enable = "sse2")]
fn mask_char_and_above(v: __m128i, len: usize) -> __m128i {
  debug_assert!(
    (2..=16).contains(&len),
    "len is outside the range 2..=16: {len}"
  );
  const MASK_REGION: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
    0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 0, 0, 0, 0, 0, 0,
  ];
  let mask =
    unsafe { _mm_loadu_si128(MASK_REGION.as_ptr().offset(16 - len as isize) as *const __m128i) };
  _mm_and_si128(v, mask)
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

pub fn str_hash_fast(bytes: &[u8]) -> u64 {
  let ptr = bytes.as_ptr();
  let v = if unlikely(unaligned_read_would_cross_page_boundary::<__m128i>(ptr)) {
    read_str_to_m128_slow(bytes)
  } else {
    unsafe { _mm_loadu_si128(ptr as *const __m128i) }
  };

  let len = bytes.len().min(16);
  let v = unsafe { mask_char_and_above(v, len) };
  let v = unsafe { compress_m128_to_u64(v) };
  scramble_u64(v)
}
