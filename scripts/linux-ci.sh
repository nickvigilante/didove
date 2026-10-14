#!/bin/sh
# Run the CI checks on Linux from a Mac (or anywhere with Docker), as an unprivileged user.
# Root would defeat the "execute bit that is not ours" test, so this creates a user.
#   docker run --rm -v "$PWD:/repo:ro" rust:1 sh /repo/scripts/linux-ci.sh
set -eu
mkdir /work && tar -C /repo --exclude=./target --exclude=./.git --exclude=./.worktrees --exclude=./experiments -cf - . | tar -C /work -xf -
useradd -m tester && chown -R tester /work /usr/local/cargo /usr/local/rustup
su tester -c '
set -eu
export PATH=/usr/local/cargo/bin:$PATH CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup
cd /work
rustup component add clippy rustfmt >/dev/null 2>&1
rustup toolchain install 1.88 --profile minimal >/dev/null 2>&1
echo "== fmt"; cargo fmt --check && echo ok
echo "== clippy"; cargo clippy --all-targets -- -D warnings 2>&1 | tail -3
echo "== test"; cargo test 2>&1 | grep -E "test result|FAILED|panicked|failed"
echo "== msrv"; cargo +1.88 check --all-targets --locked 2>&1 | tail -2
echo "== release + musl static"
rustup target add $(uname -m)-unknown-linux-musl >/dev/null 2>&1
cargo build --release --target $(uname -m)-unknown-linux-musl 2>&1 | tail -1
stat -c "%s bytes: %n" target/$(uname -m)-unknown-linux-musl/release/dv
'
