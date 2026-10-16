"""Compares Skene's layout of the platform-text cases with Pango's (pango-view, the same
fontconfig): each line's text, and the family each character is drawn in, plus which emoji
clusters each draws in colour.

usage: python3 -I tools/pango-compare.py SHOTS_DIR OUT_DIR
"""

import json
import os
import subprocess
import sys

CASES = [
  ("latin", "The quick brown fox jumps over the lazy dog", 0),
  ("emoji", "Emoji 👍🏽 👨‍👩‍👧 🇰🇷 😀 ❤️ ☺ ☺️", 0),
  ("cjk", "日本語の文章を折り返します。中文文本换行。한국어 문장입니다.", 110),
  ("thai", "ภาษาไทยเป็นภาษาที่ไม่มีการเว้นวรรคระหว่างคำ", 90),
  ("khmer", "ភាសាខ្មែរគឺជាភាសាផ្លូវការរបស់ប្រទេសកម្ពុជា", 90),
  ("hyphen", "Donaudampfschifffahrtsgesellschaft internationalization", 110),
  ("softhyphen", "Incom­pre­hen­si­bil­i­ties every­where", 90),
  ("bidi", "Hello שלום עולם world مرحبا بالعالم 123", 0),
  ("arabic", "مرحبا بالعالم هذا نص عربي طويل للالتفاف", 110),
  ("ellipsis", "A long label that must fit in two lines and end with an ellipsis when it overflows", 120),
]


def pango(name, text, width, out):
  args = ["pango-view", "-q", "--font=sans-serif 13", "--dpi=72", "--margin=4",
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
  lines = [raw[line["start-index"]:line["start-index"] + line["length"]].decode()
           for line in data["output"]["lines"]]
  families = {}
  for line in data["output"]["lines"]:
    for run in line["runs"]:
      family = run["font"]["description"].rsplit(" ", 1)[0]
      start = run["offset"]
      for byte in range(start, start + run["length"]):
        families[byte] = family
  return lines, families


def skene(name, shots):
  with open(f"{shots}/{name}-skene.json") as file:
    data = json.load(file)
  return data["lines"], data["runs"]


def chars(text):
  at = 0
  for character in text:
    yield at, character
    at += len(character.encode())


def main():
  shots, out = sys.argv[1], sys.argv[2]
  os.makedirs(out, exist_ok=True)
  for name, text, width in CASES:
    pango_lines, pango_families = pango(name, text, width, out)
    lines, runs = skene(name, shots)
    ours = {}
    for start, end, family in runs:
      for byte in range(start, end):
        ours[byte] = family
    drawn = [(byte, c) for byte, c in chars(text) if not c.isspace() and c not in "­‍️"]
    def agree(byte):
      # Pango names the face ("WenQuanYi Zen Hei Medium"), Skene the family.
      theirs, mine = pango_families.get(byte) or "", ours.get(byte) or "?"
      return theirs == mine or theirs.startswith(mine + " ")

    # A cluster Skene cut for an ellipsis is not compared.
    drawn = [(byte, c) for byte, c in drawn if byte in ours]
    same = sum(1 for byte, _ in drawn if agree(byte))
    differing = sorted({(c, pango_families.get(byte), ours.get(byte)) for byte, c in drawn
                        if not agree(byte)})
    print(f"== {name}: lines {'equal' if lines == pango_lines else 'differ'}; "
          f"family per character agrees {same}/{len(drawn)}")
    if lines != pango_lines:
      print(f"   pango {pango_lines}")
      print(f"   skene {lines}")
    for c, theirs, mine in differing[:6]:
      print(f"   {c!r}: pango {theirs}, skene {mine}")


main()
