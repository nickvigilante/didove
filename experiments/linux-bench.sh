#!/bin/sh
# Run inside a Rust container, from the experiments/ directory:
#   docker run --rm -v "$PWD:/exp:ro" -v "$PWD/results:/out" rust:1 sh /exp/linux-bench.sh
set -eu
arch=$(uname -m)
mkdir /work && tar -C /exp/path-search --exclude=./target -cf - . | tar -C /work -xf - && cd /work
rustup target add "$arch-unknown-linux-musl" >/dev/null 2>&1
apt-get update -qq >/dev/null && apt-get install -y -qq hyperfine busybox-static >/dev/null

echo "== tests (unprivileged) =="
useradd -m tester
mkdir -p /fixture/a /fixture/b
printf '#!/bin/sh\n' > /fixture/a/tool; printf '#!/bin/sh\n' > /fixture/b/tool
chmod 0700 /fixture/a/tool; chmod 0755 /fixture/b/tool; chmod 0755 /fixture /fixture/a /fixture/b
chown -R tester /work; chown -R tester /usr/local/cargo /usr/local/rustup 2>/dev/null || true
su tester -c "cd /work && PATH_SEARCH_OTHERUSER_FIXTURE=/fixture CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup /usr/local/cargo/bin/cargo test --features scenarios -- --nocapture 2>&1 | grep -E 'test |skipped|result'"

echo "== builds =="
cargo build -q --release
RUSTFLAGS='-C target-feature=+crt-static' cargo build -q --release --target "$arch-unknown-linux-gnu" --target-dir target-gnu-static
cargo build -q --release --target "$arch-unknown-linux-musl" --target-dir target-musl
mkdir -p bins
for b in walk_bin which_bin; do
  cp "target/release/$b" "bins/$b.gnu-dyn"
  cp "target-gnu-static/$arch-unknown-linux-gnu/release/$b" "bins/$b.gnu-static"
  cp "target-musl/$arch-unknown-linux-musl/release/$b" "bins/$b.musl-static"
done
echo "== sizes (bytes) =="; stat -c '%s %n' bins/*
echo "== linkage =="; for f in bins/walk_bin.*; do echo "$f: $(file -b "$f" | cut -c1-90)"; done

echo "== sanity (output and exit code of every binary) =="
echo "PATH has $(echo "$PATH" | tr ':' '\n' | wc -l) entries: $PATH"
for f in bins/*; do
  a=0; out=$($f cargo) || a=$?
  b=0; $f nosuchcmd-xyz >/dev/null || b=$?
  echo "$f: cargo -> '$out' (exit $a); missing -> exit $b"
done

echo "== bench =="
args=""
for v in gnu-dyn gnu-static musl-static; do
  args="$args -n walk.$v:cargo 'bins/walk_bin.$v cargo' -n which-crate.$v:cargo 'bins/which_bin.$v cargo'"
  args="$args -n walk.$v:missing 'bins/walk_bin.$v nosuchcmd-xyz' -n which-crate.$v:missing 'bins/which_bin.$v nosuchcmd-xyz'"
done
args="$args -n usr-bin-which:cargo '/usr/bin/which cargo' -n usr-bin-which:missing '/usr/bin/which nosuchcmd-xyz'"
args="$args -n busybox-static-which:cargo '/bin/busybox which cargo' -n busybox-static-which:missing '/bin/busybox which nosuchcmd-xyz'"
args="$args -n sh-command-v:cargo \"/bin/sh -c 'command -v cargo'\" -n sh-command-v:missing \"/bin/sh -c 'command -v nosuchcmd-xyz'\""
args="$args -n sh-baseline \"/bin/sh -c ':'\" -n true-baseline /bin/true"
eval hyperfine -i --warmup 50 -N --runs 1000 --export-json /out/linux-$arch.json $args >/dev/null 2>&1
echo done
