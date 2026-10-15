#!/usr/bin/env python3
"""Turns the emulator's `run=R test=T key=value` lines into the spike's criteria report."""

import statistics
import sys
from collections import defaultdict

values = defaultdict(list)

for line in open(sys.argv[1], encoding="utf-8"):
    parts = line.strip().split(" ", 2)

    if len(parts) < 3 or "=" not in parts[2]:
        continue

    key, _, value = parts[2].partition("=")
    values[key].append(value)


def num(key):
    found = [float(v) for v in values.get(key, []) if v.replace(".", "", 1).replace("-", "", 1).isdigit()]
    return statistics.median(found) if found else None


def most(key):
    found = [float(v) for v in values.get(key, [])]
    return max(found) if found else None


def show(v):
    if v is None:
        return "n/a"

    return f"{v:.3f}" if v != int(v) else str(int(v))


print(f"emulator: sdk={','.join(values.get('emulator.sdk', ['?']))} model={','.join(values.get('emulator.model', ['?']))} density={','.join(values.get('emulator.density', ['?']))}")
print(f"instrument runs ok: {sum(int(v) for v in values.get('instrument.ok', []))} of {len(values.get('instrument.ok', []))}")
print(f"leaves: {show(num('taffy.leaves'))}; taffy runs: {len(values.get('taffy.cold.ms', []))}; compose runs: {len(values.get('compose.cold.ms', []))}")
print()
print("TIMINGS (median across processes, ms; warm passes are medians within a process first)")
print(f"{'pass':<28}{'taffy+JNI':>12}{'compose':>12}{'ratio':>9}")

ratios = {}

for name in ["cold", "widthChange", "backToFirstWidth", "alternate", "newWidth"]:
    t = num(f"taffy.{name}.ms")
    c = num(f"compose.{name}.ms")
    r = t / c if t is not None and c else None
    ratios[name] = r
    print(f"{name:<28}{show(t):>12}{show(c):>12}{show(r):>9}")

for name in ["warmSameWidth", "javaCacheOnly", "javaCacheOnlyWithIntrinsics"]:
    print(f"{name:<28}{show(num(f'taffy.{name}.ms')):>12}{'-':>12}{'-':>9}")

print(f"{'compose sameWidthNoop':<28}{'-':>12}{show(num('compose.sameWidthNoop.ms')):>12}")
print(f"taffy tree creation: {show(num('taffy.create.ms'))} ms; compose composition: {show(num('compose.composition.ms'))} ms")
print(f"root height at W1: taffy {show(num('taffy.rootHeightAtW1'))}, compose {show(num('compose.rootHeightAtW1'))}")
print()
print("JNI CROSSINGS PER PASS (max across processes)")
print(f"{'pass':<28}{'total':>8}{'intr':>7}{'layout':>8}{'maxLeaf':>9}{'>1':>6}{'leaves':>8}{'measures':>10}")

after_warmup = ["warmSameWidth", "widthChange", "backToFirstWidth", "alternate", "newWidth", "javaCacheOnly"]

for name in ["cold"] + after_warmup + ["javaCacheOnlyWithIntrinsics"]:
    k = f"taffy.{name}."
    print(f"{name:<28}{show(most(k + 'crossings')):>8}{show(most(k + 'intrinsicsCalls')):>7}{show(most(k + 'layoutCalls')):>8}{show(most(k + 'maxPerLeaf')):>9}{show(most(k + 'leavesOver1')):>6}{show(most(k + 'leavesCrossing')):>8}{show(most(k + 'measureCallbacks')):>10}")

print()
print("OFF MAIN THREAD")

for key in sorted(k for k in values if k.startswith("offMain.")):
    print(f"  {key} = {','.join(values[key])}")

print()
print("INK VS MEASURED HEIGHT")

for key in sorted(k for k in values if k.startswith("ink.")):
    print(f"  {key} = {','.join(values[key])}")

print()
print("CRITERIA")

a_values = [most(f"taffy.{n}.maxPerLeaf") for n in after_warmup]
a_ok = all(v is not None and v <= 1 for v in a_values)
print(f"(a) <=1 JNI crossing per leaf per pass after warm-up: {'PASS' if a_ok else 'FAIL'} "
      f"(max per leaf in {', '.join(after_warmup)} = {[show(v) for v in a_values]}; "
      f"crossings per pass = {[show(most(f'taffy.{n}.crossings')) for n in after_warmup]})")

b_keys = ["cold", "widthChange", "alternate"]
b_ok = all(ratios[k] is not None and ratios[k] <= 2 for k in b_keys)
print(f"(b) within 2x of Compose (cold, width change, warm alternate): {'PASS' if b_ok else 'FAIL'} "
      f"(ratios {', '.join(f'{k}={show(ratios[k])}' for k in b_keys)})")

c_keys = [k for k in values if k.startswith("offMain.") and k.endswith(("mismatched", "Mismatched"))]
c_ok = bool(c_keys) and all(int(v) == 0 for k in c_keys for v in values[k])
print(f"(c) off-main measuring matches main: {'PASS' if c_ok else 'FAIL'} "
      f"({', '.join(f'{k}={values[k][0]}' for k in sorted(c_keys))})")

d_keys = ["ink.boringOn.heightMismatchVsFreshLayout", "ink.boringOn.inkAboveBox", "ink.boringOn.inkBelowBox"]
d_detail = ", ".join(k.split(".")[-1] + "=" + values.get(k, ["n/a"])[0] for k in d_keys)
d_ok = all(k in values and int(values[k][0]) == 0 for k in d_keys)
print(f"(d) measured height == drawn StaticLayout extent (default engine): {'PASS' if d_ok else 'FAIL'} "
      f"({d_detail}; "
      f"worst below {values.get('ink.boringOn.worstBelowPx', ['n/a'])[0]}px, above {values.get('ink.boringOn.worstAbovePx', ['n/a'])[0]}px)")
