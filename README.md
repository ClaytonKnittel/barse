# Barse - a fast solution to the 1 Billion Row Challenge

My submission to the [one billion row challenge](https://github.com/gunnarmorling/1brc) (1brc) in Rust.

## Overview

My submission conforms to the rules laid out in [ClaytonKnittel/1brc](https://github.com/ClaytonKnittel/1brc). It
~should~ compile and run on any Unix system, though some optimizations are only available on Linux with an Intel CPU
that supports the AVX2 feature.

## Performance

These numbers were collected on my i9-13900K CPU on random input files generated from
[ClaytonKnittel/1brc](https://github.com/ClaytonKnittel/1brc/tree/main/src).

|-----------------|------------------------|
| Features | Average total walltime |
|-----------------|------------------------|
| none | 5.00 s |
| "multithreaded" | 630 ms |
|-----------------|------------------------|

## Implementation Details

### Scanner

The implementation centers around the `Scanner` struct, which reads 64 bytes from the file at a time and records the
locations of the ';' and '\n' characters in those 64 bytes.
