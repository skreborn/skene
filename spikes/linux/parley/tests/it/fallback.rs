//! Linux's fallback: each character's family through fontconfig, emoji first for an
//! emoji-presentation cluster, as Pango resolves the platform-text cases.

use skene_foundation::text::ParagraphStyle;

use crate::paragraph;

fn families(text: &str) -> Vec<(String, String)> {
  use skene_foundation::text::Paragraph as _;

  paragraph(text, ParagraphStyle::default())
    .layout(None)
    .run_families()
    .into_iter()
    .map(|(range, family)| (text[range].to_owned(), family))
    .collect()
}

fn family_of(families: &[(String, String)], piece: &str) -> String {
  families
    .iter()
    .find(|(text, _)| text.trim() == piece)
    .map(|(_, family)| family.clone())
    .unwrap_or_else(|| panic!("{piece} in {families:?}"))
}

#[test]
fn emoji_presentation_goes_to_the_emoji_family_and_text_presentation_does_not() {
  let families = families(
    "Emoji \u{1F44D}\u{1F3FD} \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} \u{1F1F0}\u{1F1F7} \u{1F600} \u{2764}\u{FE0F} \u{263A} \u{263A}\u{FE0F}",
  );
  let emoji = skene_parley::linux::Fallback::load()
    .expect("fontconfig loads")
    .fontconfig()
    .resolve_family("emoji")
    .expect("an emoji family");

  for piece in [
    "\u{1F44D}\u{1F3FD}",
    "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
    "\u{1F1F0}\u{1F1F7}",
    "\u{1F600}",
    "\u{2764}\u{FE0F}",
    "\u{263A}\u{FE0F}",
  ] {
    assert_eq!(family_of(&families, piece), *emoji, "{piece}");
  }

  assert_ne!(
    family_of(&families, "\u{263A}"),
    *emoji,
    "text presentation stays text"
  );
}

#[test]
fn a_script_the_text_family_lacks_falls_back_per_character() {
  let families = families("Hello \u{5E9}\u{5DC}\u{5D5}\u{5DD} \u{65E5}\u{672C}");
  let latin = family_of(&families, "Hello");

  assert_ne!(family_of(&families, "\u{5E9}\u{5DC}\u{5D5}\u{5DD}"), latin);
  assert_ne!(family_of(&families, "\u{65E5}\u{672C}"), latin);
}
