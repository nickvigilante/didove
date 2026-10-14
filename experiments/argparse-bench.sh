#!/bin/sh
# docker run --rm -v "$PWD:/exp:ro" -v "$PWD/results:/out" rust:1 sh /exp/argparse-bench.sh
set -eu
arch=$(uname -m)
mkdir /work && tar -C /exp/argparse-startup --exclude=./target -cf - . | tar -C /work -xf - && cd /work
rustup target add "$arch-unknown-linux-musl" >/dev/null 2>&1
apt-get update -qq >/dev/null && apt-get install -y -qq hyperfine >/dev/null 2>&1
cargo build -q --release --target "$arch-unknown-linux-musl"
B="target/$arch-unknown-linux-musl/release"
stat -c '%s %n' "$B"/*_bin
hyperfine -i --warmup 50 -N --runs 2000 --export-json /out/argparse-linux.json \
  -n bare "$B/bare_bin -1 --json cargo" -n lexopt "$B/lexopt_bin -1 --json cargo" \
  -n clap "$B/clap_bin -1 --json cargo" -n clap-help "$B/clap_bin --help" >/dev/null 2>&1
