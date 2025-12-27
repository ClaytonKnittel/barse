use barse::temperature_reading_summaries;
use clap::Parser;
use itertools::Itertools;

use crate::error::BarseResult;

pub mod barse;
pub mod error;
pub mod inline_string;
pub mod scanner;
pub mod str_hash;
pub mod temperature_reading;

#[derive(Parser, Debug)]
struct Args {
  #[arg(long, default_value = "measurements.txt")]
  input: String,
}

pub fn run_parser() -> BarseResult {
  let args = Args::try_parse()?;

  println!(
    "{{{}}}",
    temperature_reading_summaries(&args.input)?
      .map(|station| format!("{station}"))
      .join(", ")
  );
  Ok(())
}
