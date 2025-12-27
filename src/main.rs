use std::process::ExitCode;

use barse::{error::BarseResult, print_summary::print_summary};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
  #[arg(long, default_value = "measurements.txt")]
  input: String,
}

pub fn run_parser() -> BarseResult {
  let args = Args::try_parse()?;
  print_summary(&args.input)
}

fn main() -> ExitCode {
  #[cfg(feature = "profiled")]
  let guard = pprof::ProfilerGuardBuilder::default()
    .frequency(1000)
    .build()
    .unwrap();

  let res = run_parser();

  #[cfg(feature = "profiled")]
  if let Ok(report) = guard.report().build() {
    let file = std::fs::File::create("brc.svg").unwrap();
    report.flamegraph(file).unwrap();
  };

  if let Err(err) = res {
    println!("{err}");
    ExitCode::FAILURE
  } else {
    ExitCode::SUCCESS
  }
}
