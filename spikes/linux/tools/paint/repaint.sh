#!/usr/bin/env bash
# What a repaint of the 1000-row probe costs on each path, at 1920x1080 and 3840x2160, on Xvfb
# and on headless mutter: MODE=full (the background behind the scroll changes, so the whole
# window repaints) and MODE=paint (one row's background changes), twelve changes a run.
# Three variants: the probe built without the GPU path (cpu-build), the probe built with it
# but asked for the CPU (cpu), and the same probe on the GPU (gpu: SKENE_RENDERER=gpu, which
# admits a software adapter such as Mesa's lavapipe).
#
#   tools/paint/repaint.sh <rows built without gpu> <rows built with gpu> <out dir> [rounds]
#
# Writes <out dir>/frames.txt (every [frame] line, prefixed by leg, size, mode, variant and
# round) and prints the medians of the frames after the first two of each run: the whole
# frame, recording the scene, and rasterizing (on the GPU: rendering, presenting and waiting).
set -u
cpu_build=$1; gpu_build=$2; out=$3; rounds=${4:-3}
here=$(cd "$(dirname "$0")/../.." && pwd)
cd "$here" || exit 1
mkdir -p "$out"; : > "$out/frames.txt"
for round in $(seq "$rounds"); do
  for leg in x11 mutter; do
    for size in 1920x1080 3840x2160; do
      for mode in full paint; do
        for variant in cpu-build cpu gpu; do
          bin=$gpu_build; renderer=$variant
          [ "$variant" = cpu-build ] && { bin=$cpu_build; renderer=cpu; }
          dir="$out/run-$leg-$size-$mode-$variant-$round"; rm -rf "$dir"; mkdir -p "$dir"
          env OUT="$dir" APP=/usr/bin/env \
            APP_ARGS="SKENE_SHOT= SKENE_RENDERER=$renderer SKENE_SIZE=$size MODE=$mode $bin" \
            SCREEN="${size}x24" MONITOR="$size" \
            timeout 120 dbus-run-session -- tools/sessions/session.sh "$leg" sleep 6.5 \
            > "$dir/session.log" 2>&1
          grep -E '^\[frame\]' "$dir/app.log" | sed "s/^/$leg $size $mode $variant $round /" >> "$out/frames.txt"
          grep -E '^\[gpu\] .*, Vulkan\)' "$dir/app.log" | head -1 | sed "s/^/$leg $size $mode $variant $round /" >> "$out/frames.txt"
        done
      done
    done
  done
done
python3 -I - "$out/frames.txt" <<'PY'
import re, statistics, sys
runs = {}
adapters = set()
for line in open(sys.argv[1]):
    parts = line.split()
    key = tuple(parts[:4])
    if parts[5] == "[gpu]":
        adapters.add(" ".join(parts[6:]))
        continue
    frame = int(parts[6])
    if frame <= 2:
        continue
    cost = dict(re.findall(r"(total|record|raster) ([0-9.]+)", line))
    runs.setdefault(key, []).append(tuple(float(cost[name]) for name in ("total", "record", "raster")))
for adapter in sorted(adapters):
    print(f"adapter: {adapter}")
print("leg size mode variant: frames, median total / record / raster ms")
for key in sorted(runs):
    frames = runs[key]
    medians = [statistics.median(frame[index] for frame in frames) for index in range(3)]
    print(f"{' '.join(key)}: {len(frames)} frames, {medians[0]:.2f} / {medians[1]:.2f} / {medians[2]:.2f}")
PY
