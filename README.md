# Barse - a fast solution to the 1 Billion Row Challenge

My submission to the [one billion row challenge](https://github.com/gunnarmorling/1brc) (1brc) in Rust.

## Overview

My submission conforms to the rules laid out in [ClaytonKnittel/1brc](https://github.com/ClaytonKnittel/1brc). It
~should~ compile and run on any Unix system, though some optimizations are only available on Linux with an Intel CPU
that supports the AVX2 feature.

## Performance

These numbers were collected on my i9-13900K CPU with an NVMe SSD on random input files generated from
[ClaytonKnittel/1brc](https://github.com/ClaytonKnittel/1brc/tree/main/src).

| Features | Average total walltime |
|-----------------|------------------------|
| none | 5.00 s |
| "multithreaded" (32 cores) | 630 ms |

## Implementation Details


### File format

The format of a line from the input file is as follows:

```
<station_name : 2 - 50 bytes>;<temperature reading><newline>
```

For example:
```
Gwanda;-26.7
Plzeň;50.9
Nardò;9.8
...
```

Temperature readings range from -99.9 to 99.9, always with one fractional digit. Station names contain valid UTF-8
characters, spanning 2 - 50 bytes.

### Scanner

The implementation centers around the `Scanner` struct, which reads from the buffered file in 64-byte batches and records the
locations of the ';' and '\n' characters in those 64 bytes.

The scanner holds a pointer to the start of the current 64-byte region in view, two bitmasks of the locations of
semicolon/newline characters in the 64-byte region, and the offset within the current 64-byte region in view of the
start of the next line.

The bitmasks are constructed directly from the file buffer using two `vpmov` reads into 32-byte `ymm` registers,
followed by `vpcmpeqb + vpmovmskb` on each. These masks are retained and used to construct all station names +
temperature readins in that 64-byte region, meaning the locations of newlines/semicolons are computed only once.

These bitmasks are used to efficiently compute the boundaries of the station name and find the start pointer of the
temperature reading to pass to the temperature reading parser.

