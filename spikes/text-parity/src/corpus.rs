//! The corpus: cases, and the variants each expands to.
//!
//! `corpus.json` beside the crate is the one source: a platform spike reads the same file
//! (or `text-parity corpus --expanded`, the variants spelled out) and enumerates the variants in
//! the same order, each case's sizes outermost and its widths innermost.

use serde::{Deserialize, Serialize};

/// The corpus as checked in.
pub const CORPUS_JSON: &str = include_str!("../corpus.json");

/// Every case, versioned so a record names the corpus it was made from.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
  /// Raised whenever a case changes.
  pub version: u32,
  /// The cases, in order.
  pub cases: Vec<Case>,
}

/// How lines align in the box.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Alignment {
  /// The paragraph's start: left in a left-to-right paragraph, right in a right-to-left one.
  Start,
  /// Centred.
  Center,
  /// The paragraph's end.
  End,
  /// Justified, the last line at the start.
  Justify,
}

/// The paragraph's base direction.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
  /// From the first strong character (UAX #9 rules P2 and P3), left to right where there is none.
  Auto,
  /// Left to right.
  Ltr,
  /// Right to left.
  Rtl,
}

/// Which hyphens an engine may draw.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hyphenation {
  /// No automatic hyphenation (a soft hyphen still breaks and draws one).
  None,
  /// Soft hyphens only: the text carries U+00AD where a word may break.
  Manual,
  /// The engine's automatic hyphenation for the locale, where it has one.
  Auto,
}

/// Line-breaking strictness, as CSS's `line-break`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LineBreak {
  /// The engine's default rules.
  Normal,
  /// The strictest rules (Japanese kinsoku: no small kana at a line start).
  Strict,
  /// The least restrictive rules.
  Loose,
  /// A break between any two grapheme clusters.
  Anywhere,
}

/// Word-level breaking, as CSS's `word-break` (with `phrase` for `auto-phrase`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WordBreak {
  /// Customary rules.
  Normal,
  /// Breaks within words allowed.
  BreakAll,
  /// No breaks within words, CJK included (Korean keeps its words whole).
  KeepAll,
  /// Breaks between phrases (Japanese), where the engine segments them.
  Phrase,
}

/// One text and how it is styled, at each of its sizes and widths.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
  /// A stable name: lowercase, digits and `-`.
  pub id: String,
  /// What the case checks.
  pub note: String,
  /// Categories the comparison totals by.
  pub tags: Vec<String>,
  /// The text, every character significant.
  pub text: String,
  /// The BCP 47 locale the text is in.
  pub locale: String,
  /// `system` (the platform's UI font), `serif`, `monospace`, or a family's name.
  pub family: String,
  /// Font sizes, in logical pixels per em (points on Apple, dp on Android, DIPs on Windows).
  pub sizes: Vec<f32>,
  /// Box widths in logical pixels; `null` lays out unconstrained (max-content).
  pub widths: Vec<Option<f32>>,
  /// The font weight, 100 to 900.
  pub weight: u16,
  /// Whether the text is italic.
  pub italic: bool,
  /// How lines align.
  pub alignment: Alignment,
  /// The base direction.
  pub direction: Direction,
  /// The most lines shown; the rest of the text is cut.
  pub max_lines: Option<u32>,
  /// Whether a cut text ends its last line with an ellipsis.
  pub ellipsis: bool,
  /// Which hyphens the engine may draw.
  pub hyphenation: Hyphenation,
  /// Line-breaking strictness.
  pub line_break: LineBreak,
  /// Word-level breaking.
  pub word_break: WordBreak,
}

/// A case at one size and one width.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Variant<'a> {
  /// The case.
  pub case: &'a Case,
  /// The font size.
  pub size: f32,
  /// The box width; `None` is unconstrained.
  pub width: Option<f32>,
}

/// A number as a variant's name spells it: `17`, `17.5`.
#[must_use]
pub fn number(value: f32) -> String {
  if value.fract() == 0. && value.abs() < 1e9 {
    #[expect(clippy::cast_possible_truncation, reason = "checked whole and small")]
    let whole = value as i64;

    whole.to_string()
  }
  else {
    value.to_string()
  }
}

impl Variant<'_> {
  /// The variant's name: `case/size/width`, the width `max` where unconstrained.
  #[must_use]
  pub fn id(&self) -> String {
    format!(
      "{}/{}/{}",
      self.case.id,
      number(self.size),
      self.width_name()
    )
  }

  /// The stem its files are named by: `case_size_width`.
  #[must_use]
  pub fn file_stem(&self) -> String {
    format!(
      "{}_{}_{}",
      self.case.id,
      number(self.size),
      self.width_name()
    )
  }

  fn width_name(&self) -> String {
    self.width.map_or_else(|| "max".to_owned(), number)
  }

  /// The variant spelled out, for an engine that reads no corpus logic of its own.
  #[must_use]
  pub fn expanded(&self) -> ExpandedVariant {
    ExpandedVariant {
      id: self.id(),
      file_stem: self.file_stem(),
      case: self.case.id.clone(),
      size: self.size,
      width: self.width,
      text: self.case.text.clone(),
      locale: self.case.locale.clone(),
      family: self.case.family.clone(),
      weight: self.case.weight,
      italic: self.case.italic,
      alignment: self.case.alignment,
      direction: self.case.direction,
      max_lines: self.case.max_lines,
      ellipsis: self.case.ellipsis,
      hyphenation: self.case.hyphenation,
      line_break: self.case.line_break,
      word_break: self.case.word_break,
    }
  }
}

/// A variant with its case's fields inlined: what `text-parity corpus --expanded` prints.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[expect(
  missing_docs,
  reason = "the fields are the case's and the variant's, documented there"
)]
pub struct ExpandedVariant {
  pub id: String,
  pub file_stem: String,
  pub case: String,
  pub size: f32,
  pub width: Option<f32>,
  pub text: String,
  pub locale: String,
  pub family: String,
  pub weight: u16,
  pub italic: bool,
  pub alignment: Alignment,
  pub direction: Direction,
  pub max_lines: Option<u32>,
  pub ellipsis: bool,
  pub hyphenation: Hyphenation,
  pub line_break: LineBreak,
  pub word_break: WordBreak,
}

/// The corpus spelled out: its version and every variant.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ExpandedCorpus {
  /// The corpus's version.
  pub version: u32,
  /// Every variant, in order.
  pub variants: Vec<ExpandedVariant>,
}

impl Corpus {
  /// The checked-in corpus.
  ///
  /// # Panics
  ///
  /// Where `corpus.json` does not parse, which a test rules out.
  #[must_use]
  pub fn builtin() -> Self {
    Self::from_json(CORPUS_JSON).expect("corpus.json parses")
  }

  /// A corpus from its JSON.
  ///
  /// # Errors
  ///
  /// Where the JSON does not hold a corpus.
  pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
    serde_json::from_str(json)
  }

  /// Every variant, each case's sizes outermost and its widths innermost.
  pub fn variants(&self) -> impl Iterator<Item = Variant<'_>> {
    self.cases.iter().flat_map(|case| {
      case.sizes.iter().flat_map(move |size| {
        case.widths.iter().map(move |width| Variant {
          case,
          size: *size,
          width: *width,
        })
      })
    })
  }

  /// The case named `id`.
  #[must_use]
  pub fn case(&self, id: &str) -> Option<&Case> {
    self.cases.iter().find(|case| case.id == id)
  }

  /// The variant named `id`.
  #[must_use]
  pub fn variant(&self, id: &str) -> Option<Variant<'_>> {
    self.variants().find(|variant| variant.id() == id)
  }

  /// Every variant spelled out.
  #[must_use]
  pub fn expanded(&self) -> ExpandedCorpus {
    ExpandedCorpus {
      version: self.version,
      variants: self.variants().map(|variant| variant.expanded()).collect(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn builtin_parses_and_names_are_unique() {
    let corpus = Corpus::builtin();
    let ids: Vec<String> = corpus.variants().map(|variant| variant.id()).collect();
    let mut unique = ids.clone();

    unique.sort();
    unique.dedup();

    assert_eq!(ids.len(), unique.len(), "variant ids are unique");
    assert!(
      corpus.cases.len() >= 60,
      "the corpus covers every required case"
    );
    assert!(
      corpus.cases.iter().all(|case| case
        .id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')),
      "case ids are file-safe"
    );
  }

  #[test]
  fn sizes_cover_the_ladder() {
    let corpus = Corpus::builtin();

    for size in [11., 13., 17., 20., 34., 64.] {
      assert!(
        corpus.variants().any(|variant| variant.size == size),
        "size {size} is in the corpus"
      );
    }
  }

  #[test]
  fn names() {
    let corpus = Corpus::builtin();
    let variant = corpus.variant("latin-kerning/17/max").expect("variant");

    assert_eq!(variant.file_stem(), "latin-kerning_17_max");
    assert_eq!(number(17.5), "17.5");
  }
}
