#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![deny(clippy::borrow_as_ptr)]

pub mod barse;
#[cfg(not(feature = "multithreaded"))]
mod build_table;
#[cfg(feature = "multithreaded")]
mod build_table_mt;
pub mod error;
mod hugepage_backed_table;
#[cfg(not(feature = "multithreaded"))]
pub mod inline_string;
#[cfg(feature = "multithreaded")]
pub mod inline_string_mt;
pub mod print_summary;
pub mod scanner;
#[cfg(not(target_feature = "avx2"))]
mod scanner_cache;
#[cfg(target_feature = "avx2")]
mod scanner_cache_x86;
#[cfg(feature = "multithreaded")]
mod shared_table;
#[cfg(feature = "multithreaded")]
mod slicer;
#[cfg(target_feature = "avx2")]
mod str_cmp_x86;
pub mod str_hash;
#[cfg(target_feature = "avx2")]
pub mod str_hash_x86;
#[cfg(not(feature = "multithreaded"))]
pub mod table;
#[cfg(not(feature = "multithreaded"))]
mod table_entry;
pub mod temperature_reading;
#[cfg(not(feature = "multithreaded"))]
mod temperature_summary;
#[cfg(feature = "multithreaded")]
mod temperature_summary_mt;
#[cfg(test)]
pub mod test_against_simple_parser;
#[cfg(test)]
pub mod test_util;
mod util;
