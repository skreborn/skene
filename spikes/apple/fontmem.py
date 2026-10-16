#!/usr/bin/env python3
"""Summarizes the font start-up and memory probe (`parity-probe fontmem STORAGE`) of one platform.

Reads DIR/fontmem-<platform>-<storage>.json (one fresh process per storage) and
DIR/fonttables-<platform>.json, and prints a markdown summary: per storage the bytes copied and
mapped, the time from `main` to the first Latin, Chinese and emoji label and the footprint after
each, the whole corpus's, each font's storage, and how many of the corpus's variants lay out
exactly as with every font copied. Usage: fontmem.py DIR PLATFORM
"""

import json
import os
import sys

DIR = sys.argv[1]
PLATFORM = sys.argv[2]
STORAGES = ("copy", "layout", "map", "map-all")


def load(path):
    try:
        with open(path, encoding="utf-8") as file:
            return json.load(file)
    except (OSError, ValueError):
        return None


def show(value, digits=1):
    return "-" if value is None else f"{value:.{digits}f}"


def main():
    runs = {storage: load(os.path.join(DIR, f"fontmem-{PLATFORM}-{storage}.json")) for storage in STORAGES}
    lines = [f"## Font start-up and memory: {PLATFORM}", ""]
    os_name = next((run.get("os") for run in runs.values() if run), None)
    lines += [f"OS: {os_name}", "",
              "Times are from `main` (the probe's process is fresh per storage); footprint is `phys_footprint`'s",
              "change from the probe's start, in MB.", "",
              "| storage | copied MB | mapped MB | register ms | Latin since main | Latin footprint | Chinese since main | Chinese step | Chinese footprint | emoji since main | emoji step | emoji footprint | corpus ms | corpus footprint | fonts | lines as copy |",
              "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    reference = (runs.get("copy") or {}).get("lines") or {}

    for storage, run in runs.items():
        if not run:
            lines.append(f"| {storage} | no run |")
            continue

        steps = {step["step"]: step for step in run.get("steps", [])}

        def step(name, key):
            return steps.get(name, {}).get(key)

        def memory(name):
            return (steps.get(name, {}).get("memory") or {}).get("phys_footprint_delta_mb")

        mine = run.get("lines") or {}
        same = sum(1 for variant, value in mine.items() if reference.get(variant) == value)
        lines.append(
            f"| {storage} | {show(run.get('copied_mb'))} | {show(run.get('mapped_mb'))} | {show(run.get('register_ms'))} | "
            f"{show(step('latin', 'since_main_ms'))} | {show(memory('latin'))} | "
            f"{show(step('chinese', 'since_main_ms'))} | {show(step('chinese', 'ms'))} | {show(memory('chinese'))} | "
            f"{show(step('emoji', 'since_main_ms'))} | {show(step('emoji', 'ms'))} | {show(memory('emoji'))} | "
            f"{show(step('corpus', 'ms'))} | {show(memory('corpus'))} | {len(run.get('fonts', []))} | {same}/{len(mine)} |")

    lines += ["", "Per step (register ms, layout ms, draw ms, copied MB, mapped MB):", ""]

    for storage, run in runs.items():
        for step in (run or {}).get("steps", []):
            if step["step"] == "corpus":
                continue

            lines.append(
                f"- {storage} {step['step']}: register {show(step.get('register_ms'))}, layout {show(step.get('layout_ms'))}, "
                f"draw {show(step.get('draw_ms'))}, copied {show(step.get('copied_mb'), 2)}, mapped {show(step.get('mapped_mb'), 2)}, "
                f"families {step.get('families')}")

    lines += ["", "Fonts as stored (MB copied / mapped; face of the file's faces; metrics rewritten; hvgl):", ""]

    for storage, run in runs.items():
        for font in (run or {}).get("fonts", []):
            access = font.get("access") or {}
            lines.append(
                f"- {storage} {font['postscript']}: {font['how']} {font['copied'] / 1e6:.2f} / {font['mapped'] / 1e6:.2f}, "
                f"face {font.get('face')}, metrics {font.get('metrics_rewritten')}, hvgl {font.get('hvgl')}, "
                f"{show(font.get('ms'))} ms, open errno {access.get('open_errno')}, mmap errno {access.get('map_errno')}, "
                f"{os.path.basename(font.get('path') or '')}")

    tables = load(os.path.join(DIR, f"fonttables-{PLATFORM}.json"))

    if tables:
        lines += ["", "The first labels' fonts (MB):", "",
                  "| font | PostScript | file | file MB | tables MB | layout tables MB | largest tables | map |",
                  "|---|---|---|---|---|---|---|---|"]

        for font in tables:
            largest = sorted(font.get("tables", []), key=lambda table: -table[1])[:4]
            access = font.get("map") or {}
            lines.append(
                f"| {font['font']} | {font['postscript']} | {os.path.basename(font.get('path') or '')} | "
                f"{show((font.get('file_bytes') or 0) / 1e6, 2)} | {show(font['tables_bytes'] / 1e6, 2)} | "
                f"{show(font['layout_bytes'] / 1e6, 3)} | {', '.join(f'{tag} {size / 1e6:.2f}' for tag, size in largest)} | "
                f"open errno {access.get('open_errno')}, mmap errno {access.get('map_errno')}, {access.get('micros')} us |")

    print("\n".join(lines))


main()
