//! The badge against the host platform: its palette resolves through a theme, and its
//! default lands at the root without one.

use acme_ui::{AcmeBadge, AcmePalette};
use skene_core::{Color, NoProperty, NodeId, Property, PropertyKey, View};
use skene_headless::{Headless, children, reset, resolve_color, text};
use skene_reactive::Owner;

const COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::Color);
const TEXT_COLOR: PropertyKey<NoProperty> = PropertyKey::Shared(Property::TextColor);

type Theme<P, C = skene_common::NoChild> = skene_common::Theme<Headless, P, C>;

fn red() -> Color {
  Color::srgb(1., 0., 0.)
}

/// The default palette's badge surface under the light scheme.
fn amber() -> Color {
  Color::srgb(0.98, 0.75, 0.18)
}

/// The default palette's badge ink under the light scheme.
fn ink() -> Color {
  Color::srgb(0.12, 0.09, 0.02)
}

/// The badge's run: the node between the container's anchors.
fn run(badge: NodeId) -> NodeId {
  children(badge)[1]
}

#[test]
fn a_badge_reads_the_theme_above_it() {
  reset();

  let owner = Owner::new();
  let scope = owner.enter(|| {
    Theme::new(AcmePalette::default().set_badge(red()))
      .set_child_builder(|| AcmeBadge::new("beta"))
      .build()
  });
  let badge = children(scope)[0];

  assert_eq!(resolve_color(badge, COLOR), Some(red()));
  assert_eq!(text(run(badge)).as_deref(), Some("beta"));
  assert_eq!(resolve_color(run(badge), TEXT_COLOR), Some(ink()));

  owner.dispose();
}

#[test]
fn a_badge_without_a_theme_paints_the_default() {
  reset();

  let owner = Owner::new();
  let badge = owner.enter(|| AcmeBadge::<Headless>::new("beta").build());

  assert_eq!(resolve_color(badge, COLOR), Some(amber()));
  assert_eq!(resolve_color(run(badge), TEXT_COLOR), Some(ink()));

  owner.dispose();
}
