"""Compares Skene's layout of the platform-text cases and the corpus cases (spikes/text-parity's)
with Pango's (pango-view, the same fontconfig): each line's text, its width without its
trailing blanks, the order its clusters are drawn in, and the family each character is drawn in.

usage: python3 -I tools/pango-compare.py SHOTS_DIR OUT_DIR
"""

import json
import os
import subprocess
import sys

SOFT_HYPHENS = ("Incom­pre­hen­si­bil­i­ties every­where, "
                "extra­or­di­nary")
THAI = "ภาษาไทยเป็นภาษาที่ไม่มีการเว้นวรรคระหว่างคำ การตัดคำจึงต้องใช้พจนานุกรม"
KHMER = "ភាសាខ្មែរគឺជាភាសាផ្លូវការរបស់ប្រទេសកម្ពុជា ហើយត្រូវបានប្រើប្រាស់ដោយប្រជាជនរាប់លាននាក់"
LAO = "ພາສາລາວເປັນພາສາທາງການຂອງສາທາລະນະລັດປະຊາທິປະໄຕປະຊາຊົນລາວ"
BURMESE = "မြန်မာဘာသာစကားသည် မြန်မာနိုင်ငံ၏ ရုံးသုံးဘာသာစကား ဖြစ်သည်။"

# The name, the text, the width it wraps at (none where zero) and the size, as verify sets them.
CASES = [
  ("latin", "The quick brown fox jumps over the lazy dog", 0, 13),
  ("emoji", "Emoji 👍🏽 👨‍👩‍👧 🇰🇷 😀 ❤️ ☺ ☺️", 0, 13),
  ("cjk", "日本語の文章を折り返します。中文文本换行。한국어 문장입니다.", 110, 13),
  ("thai", "ภาษาไทยเป็นภาษาที่ไม่มีการเว้นวรรคระหว่างคำ", 90, 13),
  ("khmer", "ភាសាខ្មែរគឺជាភាសាផ្លូវការរបស់ប្រទេសកម្ពុជា", 90, 13),
  ("hyphen", "Donaudampfschifffahrtsgesellschaft internationalization", 110, 13),
  ("softhyphen", "Incom­pre­hen­si­bil­i­ties every­where", 90, 13),
  ("bidi", "Hello שלום עולם world مرحبا بالعالم 123", 0, 13),
  ("arabic", "مرحبا بالعالم هذا نص عربي طويل للالتفاف", 110, 13),
  ("ellipsis", "A long label that must fit in two lines and end with an ellipsis when it overflows",
   120, 13),
  ("soft-hyphens-90", SOFT_HYPHENS, 90, 17),
  ("soft-hyphens-140", SOFT_HYPHENS, 140, 17),
  ("bidi-numbers", "Order 12345 שולם ב-15/10/2026 בסך ₪1,234.50 (כולל מע״מ) total", 150, 17),
  ("bidi-ltr-base", "Hello שלום עולם world مرحبا بالعالم 123", 120, 17),
  ("bidi-rtl-base", "مرحبا Hello 123 world عالم!", 100, 17),
  ("thai-90", THAI, 90, 17),
  ("thai-160", THAI, 160, 17),
  ("khmer-90", KHMER, 90, 17),
  ("khmer-160", KHMER, 160, 17),
  ("lao-90", LAO, 90, 17),
  ("lao-160", LAO, 160, 17),
  ("burmese-90", BURMESE, 90, 17),
  ("burmese-160", BURMESE, 160, 17),
  ("devanagari", "हिन्दी भारत की राजभाषा है। क्षत्रिय, ज्ञान, श्री, द्वार, कृष्ण", 120, 17),
  ("tabs", "col1\tcol2\tcol3\n1\t22\t333", 0, 17),
  ("combining", "Stacked: á̂̃ ǭ ẋ̣ q̣̇ Z͓͑͒a͔͕l͖g͗o é ñ", 0, 17),
]

# Characters neither engine draws as a cluster of its own everywhere: the soft hyphen, the
# joiners and variation selectors, the newline.
INVISIBLE = "­‍‌️︎\n"


def pango(name, text, width, size, out):
  args = ["pango-view", "-q", f"--font=sans-serif {size}", "--dpi=72", "--margin=4",
          "--background=white", "--wrap=word-char",
          f"--serialize-to={out}/{name}-pango.json", "-o", f"{out}/{name}-pango.png",
          "--text", text]
  if width:
    args.append(f"--width={width}")
  if name == "ellipsis":
    args += ["--ellipsize=end", "--height=-2"]
  subprocess.run(args, check=True, env={**os.environ, "LANG": "C.UTF-8"})
  with open(f"{out}/{name}-pango.json") as file:
    data = json.load(file)
  raw = text.encode()
  lines, widths, visual, ends = [], [], [], []
  families = {}
  for line in data["output"]["lines"]:
    start, length = line["start-index"], line["length"]
    lines.append(raw[start:start + length].decode())
    # The line's blanks at its logical end, which hang and are left out of its width.
    content = raw[start:start + length].decode().rstrip(" \t\n")
    trailing = start + len(content.encode())
    ends.append(trailing)
    width, order = 0, []
    for run in line["runs"]:
      family = run["font"]["description"].rsplit(" ", 1)[0]
      for byte in range(run["offset"], run["offset"] + run["length"]):
        families[byte] = family
      for glyph in run["glyphs"]:
        byte = run["offset"] + glyph["log-cluster"]
        if byte < trailing:
          width += glyph["width"]
        if glyph.get("is-cluster-start", False) and (not order or order[-1] != byte):
          order.append(byte)
    widths.append(width / 1024)
    visual.append(order)
  return lines, widths, visual, ends, families


def skene(name, shots):
  with open(f"{shots}/{name}-skene.json") as file:
    data = json.load(file)
  return data["lines"], data["widths"], data["visual"], data["runs"]


def chars(text):
  at = 0
  for character in text:
    yield at, character
    at += len(character.encode())


def drawn(text, order, other, end):
  """The clusters of `order` that draw something before the line's trailing blanks (`end`,
  which Pango draws at no width) and that `other` starts a cluster at too (the engines group a
  cluster's characters differently), in order."""
  raw = text.encode()
  kept = []
  for byte in order:
    character = raw[byte:].decode(errors="ignore")[:1]
    if character and character not in INVISIBLE and byte in other and byte < end:
      kept.append(byte)
  return kept


def main():
  shots, out = sys.argv[1], sys.argv[2]
  os.makedirs(out, exist_ok=True)
  totals = {"lines": 0, "order": 0, "widths": 0}
  for name, text, width, size in CASES:
    pango_lines, pango_widths, pango_visual, pango_ends, pango_families = pango(
      name, text, width, size, out)
    lines, widths, visual, runs = skene(name, shots)
    ours = {}
    for start, end, family in runs:
      for byte in range(start, end):
        ours[byte] = family
    drawn_chars = [(byte, c) for byte, c in chars(text) if not c.isspace() and c not in "­‍️"]

    def agree(byte):
      # Pango names the face ("WenQuanYi Zen Hei Medium"), Skene the family.
      theirs, mine = pango_families.get(byte) or "", ours.get(byte) or "?"
      return theirs == mine or theirs.startswith(mine + " ")

    # A cluster Skene cut for an ellipsis is not compared.
    drawn_chars = [(byte, c) for byte, c in drawn_chars if byte in ours]
    same = sum(1 for byte, _ in drawn_chars if agree(byte))
    differing = sorted({(c, pango_families.get(byte), ours.get(byte)) for byte, c in drawn_chars
                        if not agree(byte)})
    # A line's paragraph separator is Skene's line's, and not Pango's.
    equal_lines = [line.rstrip("\n") for line in lines] == pango_lines
    # Pango sets each glyph's advance at whole pixels; Skene keeps fractions.
    width_gaps = [abs(a - b) for a, b in zip(widths, pango_widths)] if equal_lines else []
    worst = max(width_gaps, default=0.)
    widths_agree = equal_lines and worst <= max(2., 0.04 * max(pango_widths, default=0.))
    same_order = equal_lines and all(
      drawn(text, a, b, end) == drawn(text, b, a, end)
      for a, b, end in zip(visual, pango_visual, pango_ends))
    totals["lines"] += equal_lines
    totals["order"] += same_order
    totals["widths"] += widths_agree
    print(f"== {name}: lines {'equal' if equal_lines else 'differ'}; "
          f"visual order {'equal' if same_order else 'differs'}; "
          f"widths {'agree' if widths_agree else 'differ'} (worst {worst:.1f}px); "
          f"family per character agrees {same}/{len(drawn_chars)}")
    if not equal_lines:
      print(f"   pango {pango_lines}")
      print(f"   skene {lines}")
    elif not widths_agree:
      print(f"   pango widths {[round(w, 1) for w in pango_widths]}")
      print(f"   skene widths {[round(w, 1) for w in widths]}")
    if equal_lines and not same_order:
      for index, (a, b, end) in enumerate(zip(visual, pango_visual, pango_ends)):
        if drawn(text, a, b, end) != drawn(text, b, a, end):
          print(f"   line {index}: pango {drawn(text, b, a, end)}")
          print(f"   line {index}: skene {drawn(text, a, b, end)}")
    for c, theirs, mine in differing[:6]:
      print(f"   {c!r}: pango {theirs}, skene {mine}")
  print(f"pango comparison: lines equal in {totals['lines']}/{len(CASES)} cases, visual order "
        f"in {totals['order']}/{len(CASES)}, widths in {totals['widths']}/{len(CASES)}")


main()
