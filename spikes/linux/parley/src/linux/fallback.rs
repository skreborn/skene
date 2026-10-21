//! Skene's own font fallback on Linux: per character, through fontconfig's sorted list, as
//! Pango falls back, with an emoji-presentation cluster resolved to the emoji family first.

use std::{ops::Range, rc::Rc};

use icu_properties::{
  CodePointSetData,
  props::{Emoji, EmojiModifier, EmojiModifierBase, EmojiPresentation, RegionalIndicator},
};
use parley_emoji::{EmojiDFA, EmojiSegmentationCategory};
use skene_foundation::{
  text::{AttributedText, RunStyle},
  value::{FontFamily, FontStyle},
};
use unicode_segmentation::UnicodeSegmentation as _;

use crate::linux::fontconfig::{Fontconfig, Loaded, Request};

/// How a cluster is presented.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Presentation {
  /// As text, in the text's own typeface where it covers it.
  Text,
  /// As an emoji: in color, from the emoji family first.
  Emoji,
}

impl Presentation {
  /// The presentation of a grapheme cluster, by the emoji properties and sequences of
  /// Unicode Technical Standard #51: a variation selector decides; else a sequence (a flag, a
  /// keycap, a modifier, a ZWJ or a tag sequence) is an emoji, and a lone character is one
  /// where it defaults to emoji presentation.
  #[must_use]
  pub fn of(cluster: &str) -> Self {
    let mut dfa = EmojiDFA::new();
    let mut leading = None;

    for character in cluster.chars() {
      let category = EmojiSegmentationCategory::from_codepoint(
        u32::from(character),
        CodePointSetData::new::<Emoji>().contains(character),
        CodePointSetData::new::<EmojiPresentation>().contains(character),
        CodePointSetData::new::<EmojiModifier>().contains(character),
        CodePointSetData::new::<EmojiModifierBase>().contains(character),
        CodePointSetData::new::<RegionalIndicator>().contains(character),
      );

      leading.get_or_insert(CodePointSetData::new::<EmojiPresentation>().contains(character));
      dfa.step_record(category);
    }

    if dfa.presentation_style(leading.unwrap_or(false)).is_emoji() {
      Self::Emoji
    } else {
      Self::Text
    }
  }
}

/// Whether `character` draws nothing of its own, so coverage is not asked of it: joiners,
/// variation selectors, tags, directional marks, the soft hyphen.
const fn is_ignorable(character: char) -> bool {
  matches!(
    character,
    '\u{AD}'
      | '\u{34F}'
      | '\u{180B}'..='\u{180F}'
      | '\u{200B}'..='\u{200F}'
      | '\u{2028}'..='\u{202E}'
      | '\u{2060}'..='\u{206F}'
      | '\u{FE00}'..='\u{FE0F}'
      | '\u{FEFF}'
      | '\u{E0000}'..='\u{E0FFF}'
  )
}

/// The fontconfig family a run's family names.
fn family_name(style: &RunStyle) -> String {
  match &style.family {
    FontFamily::System => "sans-serif".to_owned(),
    FontFamily::Monospace => "monospace".to_owned(),
    FontFamily::Named(name) => name.to_string(),
  }
}

/// The resolver: fontconfig, loaded once.
pub struct Fallback {
  fontconfig: Fontconfig,
}

impl Fallback {
  /// The resolver over the user's fontconfig configuration, `None` where fontconfig loads none.
  #[must_use]
  pub fn load() -> Option<Self> {
    Fontconfig::load().map(|fontconfig| Self { fontconfig })
  }

  /// The resolver over the configuration `loaded`.
  #[must_use]
  pub fn from_loaded(loaded: Loaded) -> Self {
    Self {
      fontconfig: Fontconfig::from_loaded(loaded),
    }
  }

  /// The configuration.
  pub const fn fontconfig(&mut self) -> &mut Fontconfig {
    &mut self.fontconfig
  }

  /// The request a run's style makes.
  #[must_use]
  pub fn request(style: &RunStyle) -> Request {
    Request {
      family: family_name(style),
      weight: style.weight.0,
      italic: style.slant == FontStyle::Italic,
    }
  }

  /// The family each stretch of `text` is drawn in: every byte covered, in order, each
  /// stretch as long as its clusters resolve alike. A blank or a control takes the text's
  /// own family where it covers it, and a cluster that draws nothing of its own (a soft
  /// hyphen, a joiner) the family before it.
  pub fn resolve(&mut self, text: &AttributedText) -> Vec<(Range<usize>, Rc<str>)> {
    let mut resolved: Vec<(Range<usize>, Rc<str>)> = Vec::new();

    for (span, style) in text.spans() {
      let request = Self::request(style);
      let sorted = self.fontconfig.sort(&request);
      let first = sorted.first();
      let value = &text.value()[span.clone()];

      // ASCII the best font covers whole resolves to it whole: every cluster is one
      // character it covers, presented as text.
      if let Some(family) = first.as_ref().filter(|_| sorted.first_covers_ascii(value)) {
        match resolved.last_mut() {
          Some((last, known)) if known == family && last.end == span.start => {
            last.end = span.end;
          }
          _ if span.is_empty() => {}
          _ => resolved.push((span, Rc::clone(family))),
        }

        continue;
      }

      let emoji = self.fontconfig.sort(&Request {
        family: "emoji".to_owned(),
        ..request.clone()
      });

      for (start, cluster) in text.value()[span.clone()].grapheme_indices(true) {
        let range = span.start + start..span.start + start + cluster.len();
        let drawn: Vec<char> = cluster
          .chars()
          .filter(|character| !is_ignorable(*character))
          .collect();
        let previous = resolved.last().map(|(_, family)| Rc::clone(family));
        let blank = cluster
          .chars()
          .all(|character| character.is_whitespace() || character.is_control());
        let family = if blank {
          let characters: Vec<char> = cluster.chars().collect();

          first
            .clone()
            .filter(|family| sorted.family_covers(family, &characters))
            .or_else(|| previous.clone())
            .or_else(|| sorted.covering(&characters))
        } else if drawn.is_empty() {
          previous.or_else(|| first.clone())
        } else if Presentation::of(cluster) == Presentation::Emoji {
          emoji
            .covering(&drawn)
            .or_else(|| sorted.covering(&drawn))
            .or_else(|| first.clone())
        } else {
          sorted
            .covering(&drawn)
            .or_else(|| sorted.covering(&drawn[..1]))
            .or_else(|| first.clone())
        };
        let Some(family) = family else {
          continue;
        };

        match resolved.last_mut() {
          Some((last, known)) if *known == family && last.end == range.start => {
            last.end = range.end;
          }
          _ => resolved.push((range, family)),
        }
      }
    }

    resolved
  }
}

#[cfg(test)]
mod tests {
  use super::Presentation;

  #[test]
  fn presentation_follows_unicode_emoji_rules() {
    for emoji in [
      "\u{1F44D}\u{1F3FD}",
      "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
      "\u{1F1F0}\u{1F1F7}",
      "\u{1F600}",
      "\u{2764}\u{FE0F}",
      "\u{263A}\u{FE0F}",
      "1\u{FE0F}\u{20E3}",
    ] {
      assert_eq!(Presentation::of(emoji), Presentation::Emoji, "{emoji}");
    }

    for text in ["\u{263A}", "a", "\u{2764}", "1", "\u{1F600}\u{FE0E}"] {
      assert_eq!(Presentation::of(text), Presentation::Text, "{text}");
    }
  }
}
