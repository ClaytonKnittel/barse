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

The input file will contain 1 billion rows, and has a maximum of 10,000 unique station names.

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

    semicolon_mask: 0x..._02_00 - semicolon at index 9
    newline_mask:   0x..._20_00 - newlines at index 13, noting that the newline at index 2 is not in the mask (we have already processed it)
    cur_offset:     3           - the next line to process starts at byte offset 3 in the current batch

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

#### For example:
| ... | b'r' | b';' | b'8' | b'.' | b'3' | b'\n' | b'S' | ... |
|---|---|---|---|---|---|---|---|---|

`                     ^ unaligned load address                     `

    // ASCII values of temperature reading digits in little-endian order:
    //               _  n  a  S  \n 3  .  8
    temp_encoding: 0x20_6E_61_53_0A_33_2E_38

To remove the garbage bytes following the temperature reading ('S' and above in the example), we can check particular
bytes in the `u64` value for the newline character, and `cmov` a bitmask depending on where the newline character is.
Then by masking the value with this bitmask, we will only be left with characters which are consistent for that
particular temperature value regardless of where it appears in the file[^temp_mask].

    mask:                 0x00_00_00_00_ff_ff_ff_ff - determined by seeing a newline character in byte index 3 of temp_encoding
    masked_temp_encoding: 0x00_00_00_00_0A_33_2E_38

Now that we have a 1-1 mapping from temperature readings to 8-byte values (as constructed above), we can find a
multiply-rightshift perfect hash offline. The idea is to essentially search for a magic number which, when multiplied by
the value constructed by reading the temperature ASCII encoding directly from the file buffer, gives a unique value in
the top `N` bits across all 2001 possible temperature encodings. We ideally want `N` to be as small as possible.

Once we have this magic number, we can construct a lookup table of size `2 ^ N` at compile time, using those top `N`
bits as the index for an encoding. The lookup table will contain pre-constructed temperature readings (e.g. `i16`
values).

I was able to find a magic number for `N = 13` (e.g. an 8192-entry table) using `examples/temp_parse.rs`.

    masked_temp_encoding:                      0x00_00_00_00_0A_33_2E_38
    magic:                                     0xD6_DF_34_36_FE_28_67_20
    product:                                   0xA9_17_83_C6_A6_BE_4F_00
    right-shift 51 bits (lookup table index):  0x00_00_00_00_00_00_15_22 (base 10: 5410)

This algorithm has ~18 cycles of latency on my Intel Raptorlake CPU: [godbolt](https://godbolt.org/z/nqs33nq8Y).

### Weather Station Table - Map of station names to temperature summaries

The next step after finding the boundaries of the weather station name string in the file buffer and parsing the
temperature reading is to lookup the weather station in a map and update its temperature summary.

The temperature summary map is a power-of-two sized hash table that uses linear probing on hash collision.

The memory backing the tables is `mmap`ped directly from the OS, and backed by hugepages on systems that support it.

The layout of the temperature summary map is different in single-threaded mode and multi-threaded mode.

#### Single-threaded layout

The table consists of an array of pairs of weather station names and tempearture summaries. The weather station names
are inlined, meaning these entries have no indirection and reference no separately allocated memory. This means we will
typically only incur one L1 cache miss to load the table entry for string comparison and temperature summary updating.

#### Multi-threaded layout

There is a single shared table consisting only of weather station names, and each thread has their own array of
temperature summaries. The index of a station in the shared table corresponds with the index of that station's
temperature summary in each thread's local temperature summary map.

Since there are far fewer unique station names than records in the input file (10k vs. 1 billion), insertions of new
station names should be uncommon, and happen mostly at the beginning of parsing as the tables are warming up. For the
majority of the program's lifetime, the table already contains all stations that will be seen, and every lookup is a
hit.

#### Synchronization of the Shared String Map

We can synchronize the shared string table using a simple spin-locking mechanism to initialize string keys when they are
newly inserted. Entries in the shared string table will have three states: empty, initializing, and initialized. We will
co-opt the length of the string to hold this state: 0 for uninitialized, -1 for initializing, and the actual string
length (some positive number) for initialized.

When a thread encounters an empty bucket while probing, it atomically swaps the length for -1. If the previous value of
the length was 0, then this thread has successfully claimed the bucket for the key it had looked up. It copies the
station name into the bucket, then atomically writes the length with `release` memory ordering.

When a thread encounters an `initializing` bucket, either by reading its length as -1 while probing, or by seeing a
previous value of -1 when attempting to claim the bucket for the key it is looking up, it needs to spin until the state
moves to `initialized`.

Once a bucket is `initialized`, its string contents have been written and will never be changed. This means threads can
do un-synchronized string comparison against this value if they see that the bucket is in the initialized state. This is
the common case, and the only synchronization required is an atomic load of the bucket state with `acquire` memory
ordering.

#### Per-thread Temperature Summary Arrays

Each thread holds their own array of temperature summaries corresponding to each weather station. The temperature
summary records the min/max/total/count of temperature readings seen for a particular station. Since updating these in
memory shared with other threads would require complicated and expensive synchronization mechanisms, duplication is the
better option. The temperature summaries are aggregated after all threads have finished executing.

### String Hashing

The string hashing algorithm is tuned for the set of weather station names in `data/weather_stations.csv`. This does not
affect the correctness of the program on arbitrary input, but does improve performance when the input file is generated
from that list.

The string hash is a balance of efficiency and quality. I found that using only the first 8 characters of the station
name is not sufficient for a high quality hash, since many weather station names share a common prefix (e.g. "College
...").

The input to the hash function is a `&str` returned from the `Scanner`, which contains a pointer to the start of the
string and a length. We do an unaligned load of 16 bytes into an `xmm` register from the start of the string (with a
fallback if this read would cross a page boundary, to prevent segfaulting) and mask off the bytes past the end of the
string[^str_mask].

The hash itself is computed by `xor`-ing the two halves of the 128-bit value into a u64, then applying a
multiply-rightshift similar to the perfect hashing scheme used for temperature parsing. However, in this case we are not
going for perfect hashing, as finding a perfect hash for a reasonably-sized table is extremely difficult with a key set
of ~45k (unique weather station names). Instead, we chose a fixed size for the hash table (determined via
experimentation), and we do an exhaustive search over all u64 values with exactly 4 bits set for the magic value. We use
the magic value that has the lowest average probing distance across random samples of 10k keys from the full data set.

#### Table sizes

In single-threaded mode, the table size that had the best performance was 1 << 20 (~1 million) entries. The size of this
table is 72 MB, which is about 3x larger than the L2 cache on my computer. The average probing distance is ~1.04,
meaning ~95% of lookups find the key in the first bucket they search.

In multi-threaded mode, the overhead per-entry in the table is much larger, since there is a copy of the temperature
summaries table per thread. The table size that had the best performance was 1 << 15 (~65k) entries. This was tuned for
a 32-thread workload. The size of this table is 35.5MB, half the size of the single-threaded table. The average probing
distance is ~1.3, much higher than the single-threaded workload. This shows that the memory working set size is much
more constraining in a multithreaded environment than compute, relative to single-threaded.

[^temp_mask]: With a clever observation, you can get away with only one conditional move when constructing this mask.
  See `TemperatureReading::u64_encoding_to_self`.
[^str_mask]: Computing this mask can be cheap even for wide registers that don't support register-wide bit shifts. The
  trick is to read from a static array at an offset determined by the length of the string. See `mask_char_and_above` in
  `src/str_cmp_x86.rs`.

