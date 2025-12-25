use barse::temperature_reading_summaries;
use clap::Parser;
use itertools::Itertools;

use crate::error::BarseResult;

pub mod barse;
pub mod error;

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
