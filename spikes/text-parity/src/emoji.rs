//! Which grapheme clusters present as emoji, by Unicode Technical Standard #51.
//!
//! Parley 0.12 appends the emoji family after a run's own families for an emoji cluster, so a
//! text font that covers the characters (`#` and U+20E3 of a keycap, `❤` before U+FE0F) draws
//! it in monochrome. Skene's engine asks the emoji family first for such a cluster, as the
//! drawn Linux prototype does and as CoreText, DirectWrite and Minikin pick a colour font for an
//! emoji-presentation sequence; [`emoji_ranges`] finds the clusters.

use icu_properties::{
  CodePointSetData,
  props::{Emoji, EmojiModifier, EmojiModifierBase, EmojiPresentation, RegionalIndicator},
};
use parley_emoji::{EmojiDFA, EmojiSegmentationCategory};
use unicode_segmentation::UnicodeSegmentation as _;

/// Whether the grapheme cluster `cluster` presents as an emoji: a variation selector decides;
/// else a sequence (a flag, a keycap, a modifier, a ZWJ or a tag sequence) is one, and a lone
/// character is one where it defaults to emoji presentation.
#[must_use]
pub fn is_emoji_presentation(cluster: &str) -> bool {
  let mut dfa = EmojiDFA::new();
  let mut leading = None;

  for character in cluster.chars() {
    let presentation = CodePointSetData::new::<EmojiPresentation>().contains(character);
    let category = EmojiSegmentationCategory::from_codepoint(
      u32::from(character),
      CodePointSetData::new::<Emoji>().contains(character),
      presentation,
      CodePointSetData::new::<EmojiModifier>().contains(character),
      CodePointSetData::new::<EmojiModifierBase>().contains(character),
      CodePointSetData::new::<RegionalIndicator>().contains(character),
    );

    leading.get_or_insert(presentation);
    dfa.step_record(category);
  }

  dfa.presentation_style(leading.unwrap_or(false)).is_emoji()
}

/// The byte ranges of `text`'s emoji-presentation clusters, adjacent ones merged.
#[must_use]
pub fn emoji_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
  let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();

  for (at, cluster) in text.grapheme_indices(true) {
    if !is_emoji_presentation(cluster) {
      continue;
    }

    match ranges.last_mut() {
      Some(last) if last.end == at => last.end = at + cluster.len(),
      _ => ranges.push(at..at + cluster.len()),
    }
  }

  ranges
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn presentation() {
    for emoji in [
      "😀",
      "❤\u{FE0F}",
      "#\u{FE0F}\u{20E3}",
      "🇯🇵",
      "👍🏽",
      "👨\u{200D}💻",
    ] {
      assert!(is_emoji_presentation(emoji), "{emoji:?} is an emoji");
    }

    for text in ["❤", "©", "#", "a", "☺"] {
      assert!(!is_emoji_presentation(text), "{text:?} is text");
    }

    assert_eq!(emoji_ranges("a 😀😀 b ❤"), vec![2..10]);
  }
}
