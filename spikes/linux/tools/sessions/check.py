"""Checks the shell scenarios make on what the app painted and logged.

  check.py pixel FILE X Y RRGGBB [TOLERANCE]   PASS where the pixel is that color
  check.py size FILE WIDTH HEIGHT               PASS where the PNG is that size
  check.py frames LOG NAME                      frame costs: the first frame, and the
                                                medians of frames repainting under a
                                                quarter of the window (a hover), more (a
                                                scroll), and the whole of it (a relayout)
  check.py grep LOG PATTERN WHAT                PASS where a line of LOG holds PATTERN

Every check prints one line starting PASS or FAIL and the thing checked, FAIL where the file it
reads is missing.
"""

import re
import statistics
import sys

from PIL import Image


def pixel(path, x, y, hex_color, tolerance="2"):
  name = path.rsplit("/", 1)[-1]

  try:
    image = Image.open(path).convert("RGBA")
  except OSError as error:
    print(f"FAIL pixel {name} ({x},{y}) is #{hex_color} (no image: {error.strerror or error})")

    return

  got = image.getpixel((int(x), int(y)))[:3]
  want = tuple(int(hex_color[i:i + 2], 16) for i in (0, 2, 4))
  ok = all(abs(a - b) <= int(tolerance) for a, b in zip(got, want))
  print(f"{'PASS' if ok else 'FAIL'} pixel {name} ({x},{y}) is #{hex_color} (got #{''.join(f'{c:02x}' for c in got)})")


def size(path, width, height):
  name = path.rsplit("/", 1)[-1]

  try:
    image = Image.open(path)
  except OSError as error:
    print(f"FAIL size {name} is {width}x{height} (no image: {error.strerror or error})")

    return

  ok = image.size == (int(width), int(height))
  print(f"{'PASS' if ok else 'FAIL'} size {name} is {width}x{height} (got {image.size[0]}x{image.size[1]})")


FRAME = re.compile(
  r"\[frame\] (\d+) total ([\d.]+) ms prepare ([\d.]+) layout ([\d.]+) record ([\d.]+) "
  r"raster ([\d.]+) glyphs ([\d.]+) (full|damage) region (\d+)x(\d+)"
)


def frames(path, name):
  rows = []

  try:
    lines = open(path, errors="replace").readlines()
  except OSError:
    lines = []

  for line in lines:
    match = FRAME.search(line)

    if match:
      rows.append(match.groups())

  if not rows:
    print(f"FAIL frames {name}: none logged")

    return

  first = rows[0]
  area = float(first[8]) * float(first[9])
  small = [
    float(row[1]) for row in rows[1:] if row[7] == "damage" and float(row[8]) * float(row[9]) < area / 4
  ]
  large = [
    float(row[1]) for row in rows[1:] if row[7] == "damage" and float(row[8]) * float(row[9]) >= area / 4
  ]
  full = [float(row[1]) for row in rows[1:] if row[7] == "full"]
  fmt = lambda values: (
    f"median {statistics.median(values):.2f} ms (n={len(values)}, max {max(values):.2f})" if values else "none"
  )

  print(
    f"FRAMES {name}: first {float(first[1]):.2f} ms (prepare {first[2]}, layout {first[3]}, record {first[4]}, "
    f"raster {first[5]}, glyphs {first[6]}, {first[8]}x{first[9]}); small damage (hover) {fmt(small)}; "
    f"large damage (a scroll) {fmt(large)}; later full frames (a relayout) {fmt(full)}"
  )


def grep(path, pattern, what):
  try:
    ok = any(pattern in line for line in open(path, errors="replace"))
  except OSError as error:
    print(f"FAIL {what} (no log: {error.strerror or error})")

    return

  print(f"{'PASS' if ok else 'FAIL'} {what}")


if __name__ == "__main__":
  command, *arguments = sys.argv[1:]
  {"pixel": pixel, "size": size, "frames": frames, "grep": grep}[command](*arguments)
