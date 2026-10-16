#!/usr/bin/env python3
"""Summarizes the advance-rule sweep into sweep.md and sweep.json.

Reads, under ROOT/api<N>/parity/d<dpi>/: the advance probe's analysis (advances.json), the host's
comparisons (cmp-*.json) and the C floor's shaping reports (shaping-linear.json,
shaping-hinted.json). Usage: sweep.py ROOT
"""

import json
import os
import sys

ROOT = sys.argv[1]
APIS = (29, 34, 35)
DENSITIES = (320, 420, 480, 560)
PAIRS = [
    ("a-simple", "b-linear"),
    ("a-simple", "b-rule"),
    ("a-simple", "b-hinted"),
    ("a-simple", "c-linear"),
    ("a-simple", "c-hinted"),
    ("a-simple", "cfloor-linear"),
    ("a-simple", "cfloor-hinted"),
    ("b-hinted", "c-hinted"),
    ("b-linear", "cfloor-linear"),
    ("b-hinted", "cfloor-hinted"),
]


def load(path):
    try:
        with open(path, encoding="utf-8") as file:
            return json.load(file)
    except (OSError, ValueError):
        return None


def show(value, digits=3):
    return "-" if value is None else f"{value:.{digits}f}"


def main():
    summary = {}
    lines = ["# Android advance-rule sweep", ""]
    rules = []
    layout = []
    floors = []
    remaining = []

    for api in APIS:
        for dpi in DENSITIES:
            base = os.path.join(ROOT, f"api{api}", "parity", f"d{dpi}")

            if not os.path.isdir(base):
                continue

            entry = {}
            advances = load(os.path.join(base, "advances.json"))
            analysis = (advances or {}).get("analysis") or {}
            entry["rule"] = analysis.get("best")
            entry["rule_totals"] = analysis.get("totals")
            entry["rule_fonts"] = [{
                key: font.get(key) for key in (
                    "name", "path", "index", "axes", "upem", "has_instructions", "best",
                    "linear_agrees", "kerning_linear", "pairs_at_17dp")
            } | {
                "rules": {rule: {"matched": value["matched"], "of": value["of"]} for rule, value in font.get("rules", {}).items()},
                "misses_of_best": font.get("rules", {}).get(font.get("best") or "", {}).get("misses"),
            } for font in analysis.get("fonts", [])]

            # chws at 17 dp: what turning it off adds back, hinted and linear.
            measured = (advances or {}).get("measured") or {}
            sizes = measured.get("sizes_dp") or []
            at17 = sizes.index(17) if 17 in sizes else None
            chws = []

            for font in measured.get("fonts", []):
                for pair in font.get("chws", []):
                    if at17 is None:
                        continue

                    def get(key):
                        values = pair.get(key) or []
                        return values[at17] if at17 < len(values) else None

                    chws.append({
                        "text": pair["text"],
                        "hinted_default": get("hinted_default"), "hinted_off": get("hinted_off"),
                        "linear_default": get("linear_default"), "linear_off": get("linear_off"),
                    })

            entry["chws_at_17dp_px"] = chws

            for font in entry["rule_fonts"]:
                best = font.get("best")
                matched = font["rules"].get(best or "", {})
                hinted = font["rules"].get("hinted", {})
                rules.append(
                    f"| {api} | {dpi} | {font['name']} | {os.path.basename(font.get('path') or '')}#{font.get('index')} | "
                    f"{font.get('has_instructions')} | {best} {matched.get('matched')}/{matched.get('of')} | "
                    f"{hinted.get('matched')}/{hinted.get('of')} | "
                    f"{(font.get('linear_agrees') or {}).get('matched')}/{(font.get('linear_agrees') or {}).get('of')} | "
                    f"{(font.get('kerning_linear') or {}).get('matched')}/{(font.get('kerning_linear') or {}).get('of')} "
                    f"(max {show((font.get('kerning_linear') or {}).get('max_delta'))}) |")

            comparisons = {}

            for a, b in PAIRS:
                data = load(os.path.join(base, f"cmp-{a}-{b}.json"))

                if data:
                    comparisons[f"{a}|{b}"] = data["totals"]

                    if (a, b) == ("a-simple", "b-hinted"):
                        entry["breaks_differ_a_b_hinted"] = [
                            variant["variant"] for variant in data["variants"] if not variant["breaks_identical"]]

            entry["comparisons"] = comparisons

            def t(a, b):
                return comparisons.get(f"{a}|{b}") or {}

            def breaks(a, b):
                found = t(a, b)
                return f"{found.get('breaks_identical', '-')}/{found.get('variants', '-')}"

            layout.append(
                f"| {api} | {dpi} | {entry['rule']} | {breaks('a-simple', 'b-linear')} | {breaks('a-simple', 'b-rule')} | "
                f"{breaks('a-simple', 'b-hinted')} | {t('a-simple', 'b-hinted').get('widths_within', '-')} | "
                f"{t('a-simple', 'b-hinted').get('heights_within', '-')} | {breaks('a-simple', 'c-hinted')} | "
                f"{show(t('a-simple', 'c-linear').get('mean_ssim'))} / {show(t('a-simple', 'c-linear').get('mean_ink_iou'))} | "
                f"{show(t('a-simple', 'c-hinted').get('mean_ssim'))} / {show(t('a-simple', 'c-hinted').get('mean_ink_iou'))} | "
                f"{show(t('a-simple', 'c-hinted').get('mean_abs_diff'), 2)} | {show(t('b-hinted', 'c-hinted').get('mean_ssim'))} |")

            shaping = {}

            for name in ("linear", "hinted"):
                data = load(os.path.join(base, f"shaping-{name}.json"))

                if data:
                    worst_line = max((variant.get("max_line_end_drift", 0) for variant in data.get("variants", [])), default=None)
                    shaping[name] = {
                        "runs": data.get("runs"), "runs_same_glyphs": data.get("runs_same_glyphs"),
                        "runs_compared": data.get("runs_compared"),
                        "max_glyph_drift": data.get("max_glyph_drift"),
                        "max_run_advance_delta": data.get("max_run_advance_delta"),
                        "max_line_end_drift": worst_line,
                        "worst_variants": sorted(
                            ((variant.get("max_line_end_drift", 0), variant["variant"]) for variant in data.get("variants", [])),
                            reverse=True)[:5],
                    }

            entry["shaping"] = shaping
            linear = shaping.get("linear", {})
            hinted = shaping.get("hinted", {})
            floors.append(
                f"| {api} | {dpi} | {show(linear.get('max_run_advance_delta'), 2)} | {show(linear.get('max_line_end_drift'), 2)} | "
                f"{show(hinted.get('max_run_advance_delta'), 2)} | {show(hinted.get('max_line_end_drift'), 2)} | "
                f"{show(hinted.get('max_glyph_drift'), 2)} | "
                f"{show(t('b-linear', 'cfloor-linear').get('mean_ssim'))} / {show(t('b-linear', 'cfloor-linear').get('mean_ink_iou'))} | "
                f"{show(t('b-hinted', 'cfloor-hinted').get('mean_ssim'))} / {show(t('b-hinted', 'cfloor-hinted').get('mean_ink_iou'))} | "
                f"{show(t('a-simple', 'cfloor-hinted').get('mean_ssim'))} | {breaks('a-simple', 'cfloor-hinted')} |")
            remaining.append(f"- API {api}, {dpi} dpi: {', '.join(entry.get('breaks_differ_a_b_hinted', []))}")
            summary[f"api{api}/d{dpi}"] = entry

    lines += [
        "## Which rule Minikin's advances follow",
        "",
        "Per font: the best rule and its matches over (glyph, size), the `hinted` rule (FreeType's",
        "default load as skrifa reproduces it), whether the measured linear advance agrees with",
        "skrifa's (the glyph is the one measured), and whether kerning is the linear scale's.",
        "",
        "| API | dpi | font | file | instructions | best | hinted | linear agrees | kerning linear |",
        "|---|---|---|---|---|---|---|---|---|",
        *rules,
        "",
        "## A against B and C (breaks identical, widths and heights within 0.5 dp, pixels)",
        "",
        "B-linear is Parley's own advances, B-rule the probe's rule, B-hinted the rule with `chws` as",
        "Minikin sets it (API 34 on); C-linear and C-hinted are their glyph runs drawn by",
        "`Canvas.drawGlyphs` (API 31 on).",
        "",
        "| API | dpi | rule | A-B linear | A-B rule | A-B hinted | widths | heights | A-C hinted | A-C linear SSIM / IoU | A-C hinted SSIM / IoU | A-C hinted MAD | B-C hinted SSIM |",
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|",
        *layout,
        "",
        "## The C floor (`drawTextRun`, Minikin reshaping each of Parley's runs)",
        "",
        "Run advance drift is Minikin's advance of a run against Parley's (dp); line-end drift sums a",
        "line's runs.",
        "",
        "| API | dpi | linear run drift | linear line drift | hinted run drift | hinted line drift | hinted glyph drift | B-Cfloor linear SSIM / IoU | B-Cfloor hinted SSIM / IoU | A-Cfloor hinted SSIM | A-Cfloor hinted breaks |",
        "|---|---|---|---|---|---|---|---|---|---|---|",
        *floors,
        "",
        "## Variants whose breaks still differ (A against B-hinted)",
        "",
        *remaining,
        "",
    ]

    with open(os.path.join(ROOT, "sweep.md"), "w", encoding="utf-8") as file:
        file.write("\n".join(lines))

    with open(os.path.join(ROOT, "sweep.json"), "w", encoding="utf-8") as file:
        json.dump(summary, file, indent=1, ensure_ascii=False, default=str)


main()
