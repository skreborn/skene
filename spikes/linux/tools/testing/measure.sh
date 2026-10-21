#!/usr/bin/env bash
# Hello's build times, binary size and startup, in a target directory of its own (removed at
# the end): clean check and build, the first check after a build, three rounds of a one-line
# change (check, then build) with incremental compilation off and three with it on (cargo's
# default for a dev build, and what an app `skene new` makes builds with), `skene build linux`
# (release) with its size stripped and not,
# and the time from the process's start to its first frame on Xvfb and on headless mutter,
# five rounds each (each in a private session; the session's own start is not counted).
#
#   measure.sh <target dir>
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
target=$1
export CARGO_TARGET_DIR=$target CARGO_INCREMENTAL=0
unset DISPLAY WAYLAND_DISPLAY
cd "$root" || exit 1
now() { date +%s%N; }
since() { echo $(( ($(now) - $1) / 1000000 )); }
rm -rf "$target"

start=$(now)
cargo check -p hello >"$target.check.log" 2>&1
echo "clean cargo check -p hello: $(since "$start") ms"
start=$(now)
cargo build -p hello >"$target.build.log" 2>&1
echo "clean cargo build -p hello: $(since "$start") ms ($(grep -c Compiling "$target.build.log") crates)"
start=$(now)
cargo check -p hello >/dev/null 2>&1
echo "the first cargo check -p hello after that build: $(since "$start") ms"

lib=hello/src/lib.rs
cp "$lib" "$target.lib.rs"
# One-line changes, three rounds, with CARGO_INCREMENTAL at $1 (described as $2).
change() {
  for round in 1 2 3; do
    sed -i "s/\"Drawn by Skene\\.[^\"]*\"/\"Drawn by Skene. Round $round $1.\"/" "$lib"
    start=$(now)
    CARGO_INCREMENTAL=$1 cargo check -p hello >/dev/null 2>&1
    check=$(since "$start")
    start=$(now)
    CARGO_INCREMENTAL=$1 cargo build -p hello >/dev/null 2>&1
    echo "a one-line change to hello/src/lib.rs, $2, round $round: cargo check $check ms, cargo build $(since "$start") ms"
  done
}
change 0 "incremental off"
# The workspace's crates checked and built once with incremental compilation on, untimed.
CARGO_INCREMENTAL=1 cargo check -p hello >/dev/null 2>&1
CARGO_INCREMENTAL=1 cargo build -p hello >/dev/null 2>&1
change 1 "incremental on"
cp "$target.lib.rs" "$lib"

(cd hello && "$root/tools/bin/skene" build linux 2>/dev/null | grep "^skene build")
binary=$target/x86_64-unknown-linux-gnu/release/hello
strip -o "$target.stripped" "$binary"
echo "release hello: $(stat -c %s "$binary") bytes, $(stat -c %s "$target.stripped") stripped; links $(ldd "$binary" | grep -cE 'xcb|xkbcommon|fontconfig|freetype') of xcb/xkbcommon/fontconfig/freetype dynamically, no GTK"

for kind in x11 mutter; do
  for round in 1 2 3 4 5; do
    out=$(mktemp -d)
    APP=none OUT=$out timeout 60 dbus-run-session -- "$root/tools/sessions/session.sh" "$kind" \
      bash -c 'log=$OUT/hello.log; start=$(date +%s%N); SKENE_TRACE=1 "$0" 2>"$log" & pid=$!
        until grep -q "^\[frame\]" "$log" 2>/dev/null; do sleep 0.001; done
        echo "FIRST $(( ($(date +%s%N) - start) / 1000 ))"
        grep -m1 "^\[frame\]" "$log" | sed "s/^/COST /"
        kill $pid' "$binary" >"$out/session.txt" 2>&1
    first=$(grep -m1 '^FIRST' "$out/session.txt" | cut -d' ' -f2)
    cost=$(grep -m1 '^COST' "$out/session.txt" | grep -oE 'total [0-9.]+ ms')
    echo "startup on $kind, round $round: first frame $(awk "BEGIN {printf \"%.1f\", ${first:-0} / 1000}") ms after the process started (the frame itself: ${cost:-?})"
    rm -rf "$out"
  done
done

echo "the measurements' own target: $(du -sh "$target" | cut -f1)"
rm -rf "$target" "$target".*
