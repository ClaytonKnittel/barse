use clap::Parser;

use crate::error::BarseResult;

pub mod barse;
pub mod error;
pub mod inline_string;
pub mod print_summary;
pub mod scanner;
pub mod str_hash;
pub mod table;
pub mod temperature_reading;
mod util;

#[derive(Parser, Debug)]
struct Args {
  #[arg(long, default_value = "measurements.txt")]
  input: String,
}

pub fn run_parser() -> BarseResult {
  let args = Args::try_parse()?;
  print_summary::print_summary(&args.input)
}
