use std::process::ExitCode;

use barse::temperature_reading_summaries;
use clap::Parser;
use itertools::Itertools;

use crate::error::BarseResult;

mod barse;
mod error;

#[derive(Parser, Debug)]
struct Args {
  #[arg(long, default_value = "measurements.txt")]
  input: String,
}

fn run() -> BarseResult {
  let args = Args::try_parse()?;

  println!(
    "{{{}}}",
    temperature_reading_summaries(&args.input)?
      .map(|station| format!("{station}"))
      .join(", ")
  );
  Ok(())
}

fn main() -> ExitCode {
  #[cfg(feature = "profiled")]
  let guard = pprof::ProfilerGuardBuilder::default()
    .frequency(1000)
    .blocklist(&["libc", "libgcc", "pthread", "vdso"])
    .build()
    .unwrap();

  let res = run();

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
