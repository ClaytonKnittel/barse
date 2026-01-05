# Barse - a fast solution to the 1 Billion Row Challenge

My submission to the [one billion row challenge](https://github.com/gunnarmorling/1brc) (1brc) in Rust.

## Overview

My submission conforms to the rules laid out in [ClaytonKnittel/1brc](https://github.com/ClaytonKnittel/1brc). It
_should_ compile and run on any Unix system, though some optimizations are only available on Linux with an Intel CPU
that supports the AVX2 feature.

## Performance

These numbers were collected on my i9-13900K CPU with an NVMe SSD on random input files generated from
[ClaytonKnittel/1brc](https://github.com/ClaytonKnittel/1brc/tree/main/src).

| Features | Average total walltime |
|-----------------|------------------------|
| none (single-threaded) | 5.00 s |
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

### File MMap

The file is direcly mmap-ed into memory and read from sequentially. I refer to this region of memory as the "file
buffer".

### Scanner - AVX for fast character search

The implementation centers around the `Scanner` struct, which reads from the file buffer in 64-byte batches and records
the locations of the ';' and '\n' characters in those 64 bytes.

The scanner holds a pointer to the start of the current 64-byte batch in view, two bitmasks of the locations of
semicolon/newline characters in the batch, and the offset within the batch of the start of the next line to be processed
from the file.

#### For example: a batch from the middle of a file
| b'.' | b'7' | b'\n' | b'D' | b'e' | b'n' | b'v' | b'e' | b'r' | b';' | b'8' | b'.' | b'3' | b'\n' | b'S' | b'a' | b'n' | ... |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|

^ Pointer to buffer (points to the start of the current batch)

    semicolon_mask: 0x..._02_00 (semicolon at index 9)
    newline_mask: 0x..._20_00 (newlines at index 13, noting that the newline at index 2 is not in the mask (we have already processed it)
    cur_offset: 3 (the next line to process starts at byte offset 3 in the current batch

The bitmasks are constructed directly from the file buffer using two `vpmov` reads into 32-byte `ymm` registers,
followed by `vpcmpeqb + vpmovmskb` on each. These masks are retained and used to efficiently compute the boundaries of
all station names + temperature readings in that 64-byte region. The locations of newlines/semicolons are computed only
once.

### Parsing Temperature Readings - Perfect Hashing

As stated above, tempearture readings range from -99.9 to 99.9, always with one fractional digit. This means temperature
readings have 2001 unique values (so 2001 unique representations).

The fast path for parsing temperature readings does an unaligned 8-byte load from a pointer to the start of the
temperature reading in the file buffer. The least-significant byte of this value will contain the ASCII encoding of the
first character of the temperature reading, and so on up to the newline character, and beyond (including the first few
bytes of the weather station name on the following line, i.e. garbage).

To remove the garbage bytes following the temperature reading, we can check particular bytes in the `u64` value for the
newline character, and `cmov` a bitmask depending on where the newline character is. Then by masking the value with this
bitmask, we will only be left with characters which are consistent for that particular temperature value regardless of
where it apperas in the file[^temp_mask].

Now that we have a 1-1 mapping from temperature readings to the 8-byte value constructed above, we can find a
multiply-rightshift perfect hash offline. The idea is to essentially search for a magic number which, when multiplied by
the value constructed by reading the temperature ASCII encoding directly from the file buffer, gives a unique value in
the top `N` bits across all 2001 possible temperature encodings. We ideally want `N` to be as small as possible.

Once we have this magic number, we can construct a lookup table of size `2 ^ N` at compile time, using those top `N`
bits as the index for an encoding. The lookup table will contain pre-constructed temperature readings (e.g. `i16`
values).

I was able to find a magic number for `N = 13` (e.g. an 8192-entry table) using `examples/temp_parse.rs`.

This algorithm has ~18 cycles of latency on my Intel Raptorlake CPU: [godbolt](https://godbolt.org/z/nqs33nq8Y).

[^temp_mask]: With a clever observation, you can get away with only one conditional move when constructing this mask.
  See `TemperatureReading::u64_encoding_to_self`.

