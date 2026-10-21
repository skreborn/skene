"""Reads the frame costs tools/bench/frames.sh collects into its frames.txt.

usage: python3 -I tools/bench/frames.py summary FRAMES_TXT
       python3 -I tools/bench/frames.py undecided FRAMES_TXT

summary prints, per leg and case, the medians over every run the file holds, then a BENCH line
for each judged case judging its first frame and that frame's layout against their targets by
the 95% interval of the median. undecided prints, a `<leg> <case>` line each, the judged cases
whose first frame or layout is still undecided against its target.
"""

import re
import statistics
import sys
from math import comb

FRAME = re.compile(
  r"\[frame\] (\d+) total ([\d.]+) ms prepare ([\d.]+) layout ([\d.]+) record ([\d.]+) "
  r"raster ([\d.]+) glyphs ([\d.]+) (full|damage) region (\d+)x(\d+)"
)
# A frame's parts, in the order FRAME's groups hold them from the second.
PARTS = ("total", "prepare", "layout", "record", "raster", "glyphs")
# The cases judged: the rows and the lazy lists, whose first frame is driven alike.
JUDGED = ("rows", "lazy", "lazy-10k", "lazy-100k")
# A judged case's targets in milliseconds: its first frame, and that frame's layout.
TARGETS = (("total", 50), ("layout", 16))


def read(path):
  """Every run's frames, by leg and case and then round, and the app-start times by leg and case."""
  runs = {}
  starts = {}

  for line in open(path):
    leg, case, round_, rest = line.split(" ", 3)

    if rest.startswith("TIMING"):
      starts.setdefault((leg, case), []).append(float(re.search(r"(\d+) ms", rest).group(1)))

      continue

    match = FRAME.search(rest)

    if match:
      runs.setdefault((leg, case), {}).setdefault(round_, []).append(match.groups())

  return runs, starts


def first_parts(rounds):
  """The parts of each round's first frame, by name."""
  firsts = [frames[0] for frames in rounds.values()]

  return {name: [float(first[index]) for first in firsts] for index, name in enumerate(PARTS, 1)}


def median(values):
  return f"{statistics.median(values):.2f} [{min(values):.2f}-{max(values):.2f}]" if values else "none"


def interval(values):
  """The 95% interval of the median, between two order statistics; None under six values."""
  values = sorted(values)
  n, k = len(values), 0

  while sum(comb(n, i) for i in range(k + 1)) / 2 ** n <= 0.025:
    k += 1

  return (values[k - 1], values[n - k]) if k else None


def judged(values, target):
  """`under`, `NOT under` or `undecided against` the target, by the median's interval."""
  bounds = interval(values)

  if bounds is None:
    return "undecided against"

  low, high = bounds

  return "under" if high < target else "NOT under" if low > target else "undecided against"


def verdict(values, target):
  bounds = interval(values)
  middle = statistics.median(values)

  if bounds is None:
    return f"{middle:.2f} ms (n={len(values)}, too few rounds for an interval: undecided against {target})"

  low, high = bounds

  return f"{middle:.2f} ms (95% interval {low:.2f}-{high:.2f}, n={len(values)}: {judged(values, target)} {target})"


def undecided(runs):
  for (leg, case), rounds in sorted(runs.items()):
    if case not in JUDGED:
      continue

    parts = first_parts(rounds)

    if any(judged(parts[name], target) == "undecided against" for name, target in TARGETS):
      print(leg, case)


def summary(runs, starts):
  verdicts = []

  for (leg, case), rounds in sorted(runs.items()):
    firsts = [frames[0] for frames in rounds.values()]
    later = [frame for frames in rounds.values() for frame in frames[1:]]
    area = float(firsts[0][8]) * float(firsts[0][9])
    parts = first_parts(rounds)

    print(
      f"{leg} {case} (runs {len(firsts)}): first frame {median(parts['total'])} ms "
      f"(prepare {median(parts['prepare'])}, layout {median(parts['layout'])}, record "
      f"{median(parts['record'])}, raster {median(parts['raster'])}, glyphs {median(parts['glyphs'])}); "
      f"app start to first frame {median(starts.get((leg, case), []))} ms"
    )

    if case in JUDGED:
      # After the first frame and the second (the window's focus shown), a frame that paints a
      # small part is a hover; one that paints much of the window, or lays it out again (a lazy
      # list realizing the rows it scrolls to), is a scroll.
      driven = [frame for frames in rounds.values() for frame in frames[2:]]
      small = [
        float(frame[1]) for frame in driven
        if frame[7] == "damage" and float(frame[8]) * float(frame[9]) < area / 4
      ]
      large = [
        float(frame[1]) for frame in driven
        if frame[7] == "full" or float(frame[8]) * float(frame[9]) >= area / 4
      ]
      print(f"{leg} {case}: hover {median(small)} ms (n={len(small)}); scroll {median(large)} ms (n={len(large)})")
      verdicts.append(
        f"BENCH {leg} {case} "
        + ", ".join(
          f"{'first frame' if name == 'total' else name} {verdict(parts[name], target)}"
          for name, target in TARGETS
        )
      )

    if case.endswith("relayout"):
      relayouts = [float(frame[1]) for frame in later if frame[7] == "full"]
      relaid = [[float(frame[index]) for frame in later if frame[7] == "full"] for index in (2, 3)]
      print(
        f"{leg} {case}: relayout {median(relayouts)} ms (n={len(relayouts)}; prepare "
        f"{median(relaid[0])}, layout {median(relaid[1])})"
      )

  print("\n".join(verdicts))


def main():
  mode, path = sys.argv[1], sys.argv[2]
  runs, starts = read(path)

  if mode == "summary":
    summary(runs, starts)
  elif mode == "undecided":
    undecided(runs)
  else:
    sys.exit(f"frames.py: unknown mode {mode!r} (summary or undecided)")


main()
