#!/usr/bin/env python3
"""Summarizes the Android text-parity run of every API level into summary.md and summary.json.

Reads, under ROOT/api<N>/: the emulator's `key=value` results (results-api<N>.txt) and the pulled
parity directory (record sets, the host's cmp-*.json comparisons, the probes' JSON). Usage:
parity.py ROOT [SIZES.txt]
"""

import json
import os
import statistics
import struct
import sys
import zlib
from collections import defaultdict

ROOT = sys.argv[1]
SIZES = sys.argv[2] if len(sys.argv) > 2 else None
PAIRS = [
    ("a-simple", "b-fed", "A vs B"),
    ("a-simple", "c", "A vs C"),
    ("b-fed", "c", "B vs C"),
    ("a-simple", "b-stock", "A vs B (fontique's own fonts)"),
    ("a-quality", "b-fed", "A (high quality, hyphenation) vs B"),
    ("a-simple", "a-quality", "A simple vs A high quality"),
    ("a-subpixel", "b-fed", "A with subpixel text vs B"),
    ("a-simple", "a-subpixel", "A vs A with subpixel text"),
    ("a-simple", "cfloor", "A vs C floor (drawTextRun)"),
    ("b-fed", "cfloor", "B vs C floor"),
    ("c", "cfloor", "C vs C floor"),
]


def load(path):
    try:
        with open(path, encoding="utf-8") as file:
            return json.load(file)
    except (OSError, ValueError):
        return None


def results(api):
    values = defaultdict(list)
    path = os.path.join(ROOT, f"api{api}", f"results-api{api}.txt")

    if not os.path.exists(path):
        return values

    for line in open(path, encoding="utf-8"):
        parts = line.strip().split(" ", 2)

        if len(parts) < 3 or "=" not in parts[2]:
            continue

        key, _, value = parts[2].partition("=")
        values[key].append(value)

    return values


def median(values, key):
    found = []

    for value in values.get(key, []):
        try:
            found.append(float(value))
        except ValueError:
            pass

    return statistics.median(found) if found else None


def first(values, key):
    found = values.get(key, [])
    return found[0] if found else None


def png_pixels(path):
    """RGBA rows of an 8-bit RGB or RGBA, non-interlaced PNG."""
    data = open(path, "rb").read()
    at = 8
    chunks = []
    width = height = depth = kind = 0

    while at < len(data):
        length, name = struct.unpack(">I4s", data[at:at + 8])
        body = data[at + 8:at + 8 + length]

        if name == b"IHDR":
            width, height, depth, kind = struct.unpack(">IIBB", body[:10])
        elif name == b"IDAT":
            chunks.append(body)

        at += 12 + length

    if depth != 8 or kind not in (2, 6):
        return width, height, None

    channels = 4 if kind == 6 else 3
    raw = zlib.decompress(b"".join(chunks))
    stride = width * channels
    rows = []
    previous = bytearray(stride)
    offset = 0

    for _ in range(height):
        method = raw[offset]
        line = bytearray(raw[offset + 1:offset + 1 + stride])
        offset += 1 + stride

        for i in range(stride):
            left = line[i - channels] if i >= channels else 0
            up = previous[i]
            corner = previous[i - channels] if i >= channels else 0

            if method == 1:
                line[i] = (line[i] + left) & 255
            elif method == 2:
                line[i] = (line[i] + up) & 255
            elif method == 3:
                line[i] = (line[i] + (left + up) // 2) & 255
            elif method == 4:
                p = left + up - corner
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - corner)
                line[i] = (line[i] + (left if pa <= pb and pa <= pc else up if pb <= pc else corner)) & 255

        rows.append(bytes(line))
        previous = line

    return width, height, (rows, channels)


def colour_pixels(path):
    """Pixels whose channels spread by more than 48: colour ink, where text is black on white."""
    if not path or not os.path.exists(path):
        return None

    _, _, decoded = png_pixels(path)

    if decoded is None:
        return None

    rows, channels = decoded
    count = 0

    for row in rows:
        for i in range(0, len(row), channels):
            r, g, b = row[i], row[i + 1], row[i + 2]

            if max(r, g, b) - min(r, g, b) > 48:
                count += 1

    return count


def records(api, name):
    data = load(os.path.join(ROOT, f"api{api}", "parity", name, "records.json"))
    return {record["variant"]: record for record in data["records"]} if data else {}


def utf16_len(text):
    return len(text.encode("utf-16-le")) // 2


def font_key(path, index, postscript, family):
    """A font as the comparison names it: its PostScript name, which a symlinked file (Android's
    DroidSans.ttf is Roboto) and a collection's faces both resolve, else its file and face."""
    if postscript:
        return postscript

    return f"{os.path.basename(path)}#{index or 0}" if path else (family or "?")


def per_unit_fonts(record, text):
    """The font of each UTF-16 unit of `text`, by `font_key`."""
    fonts = [None] * utf16_len(text)

    for run in record.get("runs", []):
        span = run.get("utf16")

        if not span:
            continue

        key = font_key(run.get("path"), run.get("index"), run.get("postscript"), run.get("family"))

        for unit in range(span[0], min(span[1], len(fonts))):
            fonts[unit] = key

    return fonts


def visible_units(text):
    units = []

    for character in text:
        visible = not (character.isspace() or 0xFE00 <= ord(character) <= 0xFE0F or ord(character) in (0x200D, 0xAD))
        units.extend([visible] * (2 if ord(character) > 0xFFFF else 1))

    return units


def main():
    summary = {"apis": {}}
    lines = ["# Android text parity: options A, B and C", ""]

    for api in (29, 34, 35):
        base = os.path.join(ROOT, f"api{api}", "parity")

        if not os.path.isdir(base):
            lines += [f"## API {api}: no parity directory", ""]
            continue

        values = results(api)
        entry = {"device": {k: first(values, f"device.{k}") for k in ("sdk", "density", "model")}}
        lines += [f"## API {api}", "", f"Device: {entry['device']}", ""]

        # Comparisons, totals and per tag.
        lines += ["### Comparisons (totals)", "",
                  "| pair | variants | breaks identical | lines equal | widths within 0.5 | heights within 0.5 | same family (chars) | mean SSIM | mean ink IoU | mean abs diff |",
                  "|---|---|---|---|---|---|---|---|---|---|"]
        comparisons = {}

        for a, b, label in PAIRS:
            data = load(os.path.join(base, f"cmp-{a}-{b}.json"))

            if not data:
                continue

            comparisons[f"{a}|{b}"] = data
            t = data["totals"]
            lines.append(
                f"| {label} | {t['variants']} | {t['breaks_identical']} | {t['lines_equal']} | {t['widths_within']} | "
                f"{t['heights_within']} | {t['same_family']}/{t['characters']} | {t['mean_ssim']:.3f} | "
                f"{t['mean_ink_iou']:.3f} | {t['mean_abs_diff']:.2f} |")
            entry.setdefault("totals", {})[label] = t

        lines.append("")

        # Per tag for A vs B and A vs C, B vs C.
        for a, b, label in PAIRS[:3]:
            data = comparisons.get(f"{a}|{b}")

            if not data:
                continue

            lines += [f"### {label} by tag", "",
                      "| tag | variants | breaks identical | same family | mean SSIM | ink IoU |", "|---|---|---|---|---|---|"]

            for tag, t in sorted(data["by_tag"].items()):
                lines.append(f"| {tag} | {t['variants']} | {t['breaks_identical']} | {t['same_family']}/{t['characters']} | "
                             f"{t['mean_ssim']:.3f} | {t['mean_ink_iou']:.3f} |")
                entry.setdefault("by_tag", {}).setdefault(label, {})[tag] = {
                    k: t[k] for k in ("variants", "breaks_identical", "same_family", "characters", "mean_ssim", "mean_ink_iou")}

            lines.append("")

        # Per case: A vs B, A vs C, B vs C.
        per_case = defaultdict(dict)

        for a, b, label in PAIRS[:3]:
            data = comparisons.get(f"{a}|{b}")

            if not data:
                continue

            grouped = defaultdict(list)

            for variant in data["variants"]:
                grouped[variant["case"]].append(variant)

            for case, variants in grouped.items():
                pixels = [p for v in variants for p in v.get("pixels", {}).values()]
                per_case[case][label] = {
                    "variants": len(variants),
                    "breaks": sum(1 for v in variants if v["breaks_identical"]),
                    "family": f"{sum(v['fonts']['same_family'] for v in variants)}/{sum(v['fonts']['characters'] for v in variants)}",
                    "max_width_delta": max(abs(v["max_width_delta"]) for v in variants),
                    "max_height_delta": max(abs(v["height_delta"]) for v in variants),
                    "max_line_height_delta": max(abs(v["max_line_height_delta"]) for v in variants),
                    "max_baseline_delta": max(abs(v["max_baseline_delta"]) for v in variants),
                    "intrinsic_delta": [v.get("intrinsic_delta") for v in variants],
                    "ssim": statistics.mean(p["ssim"] for p in pixels) if pixels else None,
                    "iou": statistics.mean(p["ink_iou"] for p in pixels) if pixels else None,
                    "mean_abs_diff": statistics.mean(p["mean_abs_diff"] for p in pixels) if pixels else None,
                    "share_over": statistics.mean(p["share_over"] for p in pixels) if pixels else None,
                    "max_diff": max(p["max_diff"] for p in pixels) if pixels else None,
                    "break_texts": [v.get("break_texts") for v in variants if not v["breaks_identical"]][:2],
                    "differing_fonts": sorted({tuple(d) for v in variants for d in v["fonts"].get("differing", [])})[:4],
                }

        if per_case:
            lines += ["### Per case", "",
                      "| case | A vs B breaks | A vs B family | A vs B Δw / Δh / Δline / Δbaseline | A vs B SSIM | A vs C SSIM / IoU / MAD / >64 | B vs C SSIM / IoU / MAD / >64 / max |",
                      "|---|---|---|---|---|---|---|"]

            def show(value, digits=3):
                return "-" if value is None else f"{value:.{digits}f}"

            for case in sorted(per_case):
                ab = per_case[case].get("A vs B", {})
                ac = per_case[case].get("A vs C", {})
                bc = per_case[case].get("B vs C", {})
                lines.append(
                    f"| {case} | {ab.get('breaks', '-')}/{ab.get('variants', '-')} | {ab.get('family', '-')} | "
                    f"{show(ab.get('max_width_delta'), 2)} / {show(ab.get('max_height_delta'), 2)} / "
                    f"{show(ab.get('max_line_height_delta'), 2)} / {show(ab.get('max_baseline_delta'), 2)} | {show(ab.get('ssim'))} | "
                    f"{show(ac.get('ssim'))} / {show(ac.get('iou'))} / {show(ac.get('mean_abs_diff'), 2)} / {show(ac.get('share_over'), 4)} | "
                    f"{show(bc.get('ssim'))} / {show(bc.get('iou'))} / {show(bc.get('mean_abs_diff'), 2)} / {show(bc.get('share_over'), 4)} / {bc.get('max_diff', '-')} |")

            lines.append("")
            entry["per_case"] = per_case

        # Fallback per character: A (StaticLayout), the matcher, B fed, B with fontique's own fonts.
        a_records = records(api, "a-simple")
        fed = records(api, "b-fed")
        stock = records(api, "b-stock")
        matcher = load(os.path.join(base, "matcher-runs.json")) or {}
        fallback = {}

        postscripts = {
            (run.get("path"), run.get("index", 0)): run.get("postscript")
            for record in list(a_records.values()) + list(fed.values())
            for run in record.get("runs", [])
            if run.get("path") and run.get("postscript")
        }
        corpus = load(os.path.join(base, "corpus.json")) or {"variants": []}
        texts = {v["id"]: v["text"] for v in corpus["variants"]}
        chosen = {}

        for variant_id in a_records:
            case = variant_id.split("/")[0]

            if case not in chosen or variant_id.endswith("/max") and not chosen[case].endswith("/max"):
                chosen[case] = variant_id

        for case, variant_id in chosen.items():
            a_record = a_records[variant_id]
            text = texts.get(variant_id)

            if text is None:
                continue

            visible = visible_units(text)
            a_fonts = per_unit_fonts(a_record, text)
            fed_fonts = per_unit_fonts(fed.get(variant_id, {}), text)
            stock_fonts = per_unit_fonts(stock.get(variant_id, {}), text)
            matched = [None] * utf16_len(text)

            for run in matcher.get(case, []):
                font = run["font"]
                key = postscripts.get((font["path"], font["index"])) or font_key(font["path"], font["index"], None, None)

                for unit in range(run["start"], min(run["end"], len(matched))):
                    matched[unit] = key

            def agree(x, y):
                both = [(p, q) for p, q, v in zip(x, y, visible) if v and p and q]
                return sum(1 for p, q in both if p == q), len(both)

            notdef_b = sum(1 for run in fed.get(variant_id, {}).get("glyph_runs", []) for g in run["glyphs"] if g["id"] == 0)
            notdef_a = sum(1 for run in a_record.get("glyph_runs", []) for g in run["glyphs"] if g["id"] == 0)
            fallback[case] = {
                "A=matcher": agree(a_fonts, matched),
                "B fed=A": agree(fed_fonts, a_fonts),
                "B fed=matcher": agree(fed_fonts, matched),
                "B stock=A": agree(stock_fonts, a_fonts),
                "notdef_A": notdef_a,
                "notdef_B": notdef_b,
                "fonts_A": sorted({f for f in a_fonts if f}),
                "fonts_B_fed": sorted({f for f in fed_fonts if f}),
                "fonts_B_stock": sorted({f for f in stock_fonts if f}),
            }

        if fallback:
            lines += ["### Fallback per visible character (UTF-16 units agreeing / compared)", "",
                      "| case | A = matcher | B fed = A | B fed = matcher | B stock = A | .notdef A | .notdef B | A's fonts | B fed's fonts | B stock's fonts |",
                      "|---|---|---|---|---|---|---|---|---|---|"]

            for case in sorted(fallback):
                f = fallback[case]
                pair = lambda p: f"{p[0]}/{p[1]}"
                lines.append(f"| {case} | {pair(f['A=matcher'])} | {pair(f['B fed=A'])} | {pair(f['B fed=matcher'])} | "
                             f"{pair(f['B stock=A'])} | {f['notdef_A']} | {f['notdef_B']} | {', '.join(f['fonts_A'])} | "
                             f"{', '.join(f['fonts_B_fed'])} | {', '.join(f['fonts_B_stock'])} |")

            totals = defaultdict(lambda: [0, 0])

            for f in fallback.values():
                for key in ("A=matcher", "B fed=A", "B fed=matcher", "B stock=A"):
                    totals[key][0] += f[key][0]
                    totals[key][1] += f[key][1]

            lines += ["", f"Totals: {dict(totals)}", ""]
            entry["fallback"] = fallback
            entry["fallback_totals"] = dict(totals)

        # Breaking in complex scripts and CJK: each engine's line starts against ICU's boundaries.
        probes = load(os.path.join(base, "probes.json")) or {}
        quality = records(api, "a-quality")
        breaks = {}

        for case, probe in sorted(probes.get("breaks", {}).items()):
            icu = set(probe.get("icu", []))
            entry_case = {"icu_boundaries": len(icu)}

            for variant_id in sorted(k for k in a_records if k.startswith(case + "/")) or sorted(k for k in fed if k.startswith(case + "/")):
                width = variant_id.split("/")[-1]
                starts = lambda record: [line["utf16"][0] for line in record.get("lines", [])] if record else None
                row = {
                    "A simple": starts(a_records.get(variant_id)),
                    "A quality": starts(quality.get(variant_id)),
                    "B (dictionaries)": starts(fed.get(variant_id)),
                    "Parley without dictionaries": probe.get("plain_parley", {}).get(width),
                }

                for name, found in list(row.items()):
                    if found:
                        row[name + " off ICU"] = [start for start in found[1:] if start not in icu]

                if "icu_phrase" in probe:
                    phrase = set(probe["icu_phrase"])
                    row["StaticLayout PHRASE"] = probe.get("staticlayout_word_style", {}).get(f"{width}_phrase")
                    row["StaticLayout NONE"] = probe.get("staticlayout_word_style", {}).get(f"{width}_none")

                    for name in ("A simple", "B (dictionaries)", "StaticLayout PHRASE"):
                        if row.get(name):
                            row[name + " off ICU phrase"] = [start for start in row[name][1:] if start not in phrase]

                entry_case[width] = row

            breaks[case] = entry_case

        if breaks:
            entry["breaks"] = breaks
            lines += ["### Line starts (UTF-16) in complex scripts and CJK", "", "```", json.dumps(breaks, indent=1), "```", ""]

        # Emoji: colour pixels in each option's image.
        emoji = {}

        for variant_id, a_record in a_records.items():
            if not variant_id.startswith("emoji") and not variant_id.startswith("mixed"):
                continue

            row = {}

            for name in ("a-simple", "b-fed", "c", "cfloor"):
                record = records(api, name).get(variant_id) if name in ("a-simple", "b-fed") else None
                image = None

                if name in ("c", "cfloor"):
                    fed_record = fed.get(variant_id)
                    image = next(iter(fed_record["images"].values()), None) if fed_record else None
                elif record:
                    image = next(iter(record["images"].values()), None)

                row[name] = colour_pixels(os.path.join(base, name, image)) if image else None

            emoji[variant_id] = row

        if emoji:
            lines += ["### Colour pixels per image (emoji)", "", "| variant | A | B | C | C floor |", "|---|---|---|---|---|"]

            for variant_id in sorted(emoji):
                row = emoji[variant_id]
                lines.append(f"| {variant_id} | {row['a-simple']} | {row['b-fed']} | {row['c']} | {row['cfloor']} |")

            lines.append("")
            entry["emoji_colour_pixels"] = emoji

        # Fonts.
        fonts = {
            "ndk": first(values, "fonts.ndk.count"),
            "ndk_ms": first(values, "fonts.ndk.ms"),
            "ndk_directories": first(values, "fonts.ndk.directories"),
            "systemFonts": first(values, "fonts.systemFonts.count"),
            "systemFonts_directories": first(values, "fonts.systemFonts.directories"),
            "fontique_ms": first(values, "fonts.fontique.ms"),
            "fontique_families": first(values, "fonts.fontique.families"),
            "fontique_rss_delta_kb": first(values, "fonts.fontique.rssDeltaKb"),
            "fontique_generics": first(values, "fonts.fontique.generics"),
            "fontique_named": first(values, "fonts.fontique.namedFonts"),
            "fed_ms": first(values, "fonts.fed.ms"),
            "fed_files": first(values, "fonts.fed.files"),
            "matcher_ns": {k: first(values, f"fonts.matcher.{k}.nsPerCall") for k in ("latin", "han", "emoji", "thai")},
            "b_fed_collection_ms": first(values, "b-fed.fontCollectionMs"),
            "b_stock_collection_ms": first(values, "b-stock.fontCollectionMs"),
            "b_fed_first_layout_ms": first(values, "b-fed.firstLayoutMs"),
            "b_stock_first_layout_ms": first(values, "b-stock.firstLayoutMs"),
            "b_fed_rss_delta_kb": first(values, "b-fed.rssDeltaKb"),
        }
        stock_json = load(os.path.join(base, "fonts-fontique.json"))

        if stock_json:
            fonts["fontique_fallbacks"] = stock_json.get("fallbacks")

        han = load(os.path.join(base, "han-locale-lists.json"))
        fonts["han_locale_lists"] = han
        entry["fonts"] = fonts
        lines += ["### Fonts", "", "```", json.dumps(fonts, indent=1, ensure_ascii=False), "```", ""]

        # Probes, shaping, contrast, scale, weight.
        for name in ("probes", "shaping"):
            data = load(os.path.join(base, f"{name}.json"))

            if data:
                if name == "shaping":
                    data = {k: v for k, v in data.items() if k != "variants"} | {
                        "per_variant": [v for v in data["variants"] if v["runs_compared"] or v["max_line_end_drift"]]}

                entry[name] = data
                lines += [f"### {name}", "", "```", json.dumps(data, indent=1, ensure_ascii=False)[:20000], "```", ""]

        for name in sorted(os.listdir(base)):
            if name.startswith(("contrast-", "font-scale-")) and name.endswith(".json"):
                data = load(os.path.join(base, name))
                entry[name[:-5]] = data
                lines += [f"### {name[:-5]}", "", "```", json.dumps(data, indent=1), "```", ""]

        weight = {k: values.get(f"fontWeight.{k}") for k in ("secure", "configuration", "textViewTypefaceWeight", "textViewPaintWeight", "plainPaintWeight")}
        contrast = {k: values.get(f"contrast.{k}") for k in ("secure", "observer", "hiddenMethod", "hardwareAccelerated")}
        scale = {k: values.get(f"fontScale.{k}") for k in ("configuration", "maxSkeneVsPlatformDp", "maxLinearVsPlatformDp")}
        entry["font_weight"] = weight
        entry["contrast_reads"] = contrast
        entry["font_scale_reads"] = scale
        lines += ["### Settings reads", "", "```", json.dumps({"weight": weight, "contrast": contrast, "scale": scale}, indent=1), "```", ""]

        # Timings.
        timing = {}

        for prefix in ("taffy", "taffyPlain", "taffyParley"):
            for name in ("create", "cold", "warmSameWidth", "widthChange", "backToFirstWidth", "alternate", "newWidth",
                         "javaCacheOnly", "relayoutNoCache", "reshape", "fonts"):
                found = median(values, f"{prefix}.{name}.ms")

                if found is not None:
                    timing[f"{prefix}.{name}"] = found

            height = first(values, f"{prefix}.rootHeightAtW1")

            if height:
                timing[f"{prefix}.rootHeightAtW1"] = height

        for name in ("compose.cold", "compose.widthChange", "compose.newWidth"):
            found = median(values, f"{name}.ms")

            if found is not None:
                timing[name] = found

        for engine in ("parley", "staticLayout"):
            for stat in ("medianMs", "maxMs", "totalMs"):
                found = median(values, f"taffyParley.animate.{engine}.{stat}")

                if found is not None:
                    timing[f"animate.{engine}.{stat}"] = found

        for name in ("c.drawMs", "cfloor.drawMs", "a-simple.ms", "b-fed.ms", "b-stock.ms"):
            found = first(values, name)

            if found is not None:
                timing[name] = found

        for set_name in ("a-simple", "b-fed", "b-stock"):
            data = load(os.path.join(base, set_name, "records.json"))

            if data:
                timing[f"{set_name}.labels"] = data.get("timings", {}).get("labels")
                timing[f"{set_name}.first_layout_ms"] = data.get("timings", {}).get("first_layout_ms")
                timing[f"{set_name}.font_collection_ms"] = data.get("timings", {}).get("font_collection_ms")
                warm = [r["timings"]["warm_ms"] for r in data["records"] if r.get("timings")]
                cold = [r["timings"]["cold_ms"] for r in data["records"] if r.get("timings")]
                timing[f"{set_name}.variant_warm_median_ms"] = statistics.median(warm) if warm else None
                timing[f"{set_name}.variant_cold_median_ms"] = statistics.median(cold) if cold else None
                timing[f"{set_name}.variant_warm_total_ms"] = sum(warm)

        entry["timings"] = timing
        lines += ["### Timings (ms; medians of the 5 processes for the trees)", "", "```", json.dumps(timing, indent=1), "```", ""]
        summary["apis"][str(api)] = entry

    if SIZES and os.path.exists(SIZES):
        sizes = open(SIZES, encoding="utf-8").read()
        summary["sizes"] = sizes
        lines += ["## Library sizes", "", "```", sizes, "```", ""]

    with open(os.path.join(ROOT, "summary.md"), "w", encoding="utf-8") as file:
        file.write("\n".join(lines))

    with open(os.path.join(ROOT, "summary.json"), "w", encoding="utf-8") as file:
        json.dump(summary, file, indent=1, ensure_ascii=False, default=str)

    print("\n".join(lines))


main()
