#![cfg_attr(not(test), deny(clippy::unwrap_used))]

pub mod barse;
#[cfg(not(feature = "multithreaded"))]
mod build_table;
#[cfg(feature = "multithreaded")]
mod build_table_mt;
pub mod error;
pub mod inline_string;
pub mod print_summary;
pub mod scanner;
#[cfg(not(target_feature = "avx2"))]
mod scanner_cache;
#[cfg(target_feature = "avx2")]
mod scanner_cache_x86;
#[cfg(feature = "multithreaded")]
mod slicer;
#[cfg(target_feature = "avx2")]
mod str_cmp_x86;
pub mod str_hash;
#[cfg(target_feature = "avx2")]
pub mod str_hash_x86;
pub mod table;
mod table_entry;
mod temp_summary;
pub mod temperature_reading;
#[cfg(test)]
pub mod test_against_simple_parser;
#[cfg(test)]
pub mod test_util;
mod util;
