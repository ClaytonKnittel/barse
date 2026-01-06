#!/usr/bin/sh

# usage: ./profile.sh <binary> [args...]

set -e

cargo b --profile profiled
rm -f perf.data
perf record -e cycles:pp -F 200 --call-graph dwarf -- $@ >/dev/null
perf script | stackcollapse-perf.pl | flamegraph.pl > brc.svg
