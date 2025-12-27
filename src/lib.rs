pub mod barse;
pub mod error;
pub mod inline_string;
pub mod print_summary;
pub mod scanner;
#[cfg(not(target_feature = "avx"))]
mod scanner_generic;
#[cfg(target_feature = "avx")]
mod scanner_x86_64;
pub mod str_hash;
pub mod table;
pub mod temperature_reading;
mod util;
