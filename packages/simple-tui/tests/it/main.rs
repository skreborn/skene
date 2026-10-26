//! The Simple library on the terminal: what the browser's scripted input cannot give,
//! the pointer resting on a control and the keys walking a picker, read as the cells the
//! terminal paints, and a preset inked in its palette's default with no theme above it.

#![cfg(skene_platform = "terminal")]

use std::{any::TypeId, time::Duration};

use skene_simple_tui::prelude::*;
use skene_tui::{
  Definition,
  Scene,
  StaticLeaf,
  Token,
  TokenName,
  TokenSet,
  TokenValue,
  platform::terminal::mount,
  prelude::*,
  testing::{self, locate, user, with_platform},
  typography,
};

/// The frame's width.
const WIDTH: u16 = 24;
/// The frame's height.
const HEIGHT: u16 = 8;

/// The fixed color `token` is defined as under the light scheme.
fn light(token: &Token<Color>) -> (u8, u8, u8) {
  match token.definition().base() {
    StaticLeaf::Fixed(color) => rgb(*color),
    StaticLeaf::Token(_) | StaticLeaf::Expr(_) => panic!("the default palette holds fixed colors"),
  }
}

/// The RGB triple of `color`, as the frame reports it.
fn rgb(color: Color) -> (u8, u8, u8) {
  let [red, green, blue, _] = color.to_rgba();
  let channel = |value: f32| {
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_sign_loss,
      reason = "the channel is clamped to 0..=1 before it is scaled to 0..=255"
    )]
    let quantized = (value.clamp(0., 1.) * 255.).round() as u8;

    quantized
  };

  (channel(red), channel(green), channel(blue))
}

/// A cell's foreground and background.
type CellColors = (Option<(u8, u8, u8)>, Option<(u8, u8, u8)>);

/// The foreground and background of the cell (`x`, `y`).
fn colors_at(x: u16, y: u16) -> CellColors {
  testing::render_to_colors(WIDTH, HEIGHT)[usize::from(y) * usize::from(WIDTH) + usize::from(x)]
}

/// The background of the cell (`x`, `y`).
fn background_at(x: u16, y: u16) -> Option<(u8, u8, u8)> {
  colors_at(x, y).1
}

/// The foreground of the cell (`x`, `y`).
fn ink_at(x: u16, y: u16) -> Option<(u8, u8, u8)> {
  colors_at(x, y).0
}

/// Mounts `view` under a default Simple theme in the light scheme.
fn mount_themed<V: View + 'static>(view: impl FnOnce() -> V + 'static) {
  testing::trigger_color_scheme(ColorScheme::Light);

  let _ = mount(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(view)
      .build()
  });
}

/// Mounts `view` under no theme in the light scheme.
fn mount_bare<V: View + 'static>(view: impl FnOnce() -> V + 'static) {
  testing::trigger_color_scheme(ColorScheme::Light);

  let _ = mount(|| View::into_any(view()).build());
}

/// Paints the frame a change starts its transitions on, then lets them settle.
fn settle() {
  let _ = testing::render_to_text(WIDTH, HEIGHT);

  testing::trigger_frame(Duration::from_secs(1));
}

/// A button's surface is the primary look's, hovering paints the hover surface,
/// focusing the control paints the focus surface on the control itself, and its label
/// is inked in the look's ink.
#[test]
fn a_buttons_surface_hover_and_focus_resolve_the_primary_look_on_the_control() {
  with_platform(|| {
    let palette = SimplePalette::default();

    mount_themed(|| {
      Column::new().set_children((
        SimpleButton::new("Save"),
        SimpleButton::new("")
          .set_accessible_name("Close")
          .set_child(Container::new()),
      ))
    });

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (x, y) = locate(&frame, "Save");
    let save = testing::find_by_role(Role::Button, "Save").expect("the button");

    assert!(
      testing::find_by_role(Role::Button, "Close").is_some(),
      "an icon button takes its accessible name"
    );
    assert_eq!(
      colors_at(x, y),
      (
        Some(light(&palette.primary.ink)),
        Some(light(&palette.primary.surface))
      ),
      "{frame}"
    );
    assert_eq!(
      background_at(x - 1, y),
      Some(light(&palette.primary.surface)),
      "the inset carries the surface"
    );

    user::hover(save);
    settle();

    assert_eq!(background_at(x, y), Some(light(&palette.primary.hover)));

    testing::trigger_mouse_move(WIDTH - 1, HEIGHT - 1);
    settle();

    assert_eq!(background_at(x, y), Some(light(&palette.primary.surface)));

    testing::trigger_key("Tab");
    settle();

    assert_eq!(testing::focused_node(), Some(save));
    assert_eq!(background_at(x, y), Some(light(&palette.primary.focus)));
  });
}

/// The cell where the last line containing `needle` shows it: in the open popup, the
/// row beneath the face that shows the same label.
fn locate_last(frame: &str, needle: &str) -> (u16, u16) {
  frame
    .lines()
    .enumerate()
    .filter_map(|(row, line)| {
      let column = line.find(needle)?;

      Some((
        u16::try_from(line[..column].chars().count()).ok()?,
        u16::try_from(row).ok()?,
      ))
    })
    .last()
    .unwrap_or_else(|| panic!("{needle:?} is not in the frame:\n{frame}"))
}

/// A picker wears the field chrome and rings while focused; its indicator follows the
/// open state; its list surface is on the card; the chosen row fills in the primary
/// look with its check and caption inked to match, and the focused row highlights.
#[test]
fn a_pickers_face_rings_and_its_rows_follow_focus_and_the_choice() {
  with_platform(|| {
    let palette = SimplePalette::default();
    let value = Signal::new("md");

    mount_themed(move || {
      Column::new().set_children((SimplePicker::new()
        .set_options((
          SimpleListboxOption::new("sm", "Small"),
          SimpleListboxOption::new("md", "Medium"),
          SimpleListboxOption::new("lg", "Large"),
        ))
        .bind_chosen(value),))
    });

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (shown_x, shown_y) = locate(&frame, "Medium");
    let (indicator_x, indicator_y) = locate(&frame, "\u{25be}");
    let face = testing::find_all_by_role(Role::ComboBox)[0];

    assert_eq!(
      colors_at(shown_x, shown_y),
      (Some(light(&palette.foreground)), Some(light(&palette.card))),
      "{frame}"
    );
    assert_eq!(ink_at(0, 0), Some(light(&palette.border)), "{frame}");
    assert_eq!(
      ink_at(indicator_x, indicator_y),
      Some(light(&palette.muted_foreground))
    );

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), Some(face));
    assert_eq!(ink_at(0, 0), Some(light(&palette.ring)));

    testing::trigger_key("ArrowDown");

    let open = testing::render_to_text(WIDTH, HEIGHT);
    let medium = testing::find_by_role(Role::ListBoxOption, "Medium").expect("the chosen row");
    let small = testing::find_by_role(Role::ListBoxOption, "Small").expect("a row");
    let (medium_x, medium_y) = locate_last(&open, "Medium");
    let (small_x, small_y) = locate_last(&open, "Small");
    let (check_x, check_y) = locate(&open, "\u{2713}");
    let chosen = (
      Some(light(&palette.primary.ink)),
      Some(light(&palette.primary.surface)),
    );

    assert!(
      medium_y > shown_y,
      "the popup opens beneath the face: {open}"
    );
    assert_eq!(testing::focused_node(), Some(medium));
    assert_eq!(
      ink_at(indicator_x, indicator_y),
      Some(light(&palette.foreground)),
      "open, the indicator inks in the foreground"
    );
    assert_eq!(ink_at(0, 0), Some(light(&palette.ring)), "open, it rings");
    assert_eq!(colors_at(medium_x, medium_y), chosen, "{open}");
    assert_eq!(check_y, medium_y, "only the chosen row is checked: {open}");
    assert_eq!(colors_at(check_x, check_y), chosen);
    assert_eq!(
      colors_at(small_x, small_y),
      (Some(light(&palette.foreground)), Some(light(&palette.card))),
      "an unchosen row is clear on the list's card surface"
    );

    testing::trigger_key("ArrowUp");

    assert_eq!(testing::focused_node(), Some(small));
    assert_eq!(
      background_at(small_x, small_y),
      Some(light(&palette.secondary.surface)),
      "the focused row highlights"
    );
    assert_eq!(colors_at(medium_x, medium_y), chosen);

    testing::trigger_key("Enter");

    let closed = testing::render_to_text(WIDTH, HEIGHT);

    assert_eq!(value.get_untracked(), "sm");
    assert!(!closed.contains("Large"), "{closed}");
    assert_eq!(locate(&closed, "Small"), (shown_x, shown_y), "{closed}");

    testing::trigger_key("ArrowDown");

    let reopened = testing::render_to_text(WIDTH, HEIGHT);
    let (small_x, small_y) = locate_last(&reopened, "Small");
    let (medium_x, medium_y) = locate_last(&reopened, "Medium");

    assert_eq!(colors_at(small_x, small_y), chosen, "{reopened}");
    assert_eq!(
      background_at(medium_x, medium_y),
      Some(light(&palette.card)),
      "{reopened}"
    );
  });
}

/// A derived variant re-inks the button in place: its surface and its label's ink follow
/// the signal, its hovered and focused surfaces are the variant's own and follow a flip
/// while they show, and the control is never rebuilt.
#[test]
fn a_derived_variant_reinks_the_button_without_a_rebuild() {
  with_platform(|| {
    let palette = SimplePalette::default();
    let variant = Signal::new(SimpleVariant::Primary);

    mount_themed(move || SimpleButton::derive_variant("Go", move || variant.get()));

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (x, y) = locate(&frame, "Go");
    let button = testing::find_by_role(Role::Button, "Go").expect("the button");

    assert_eq!(
      colors_at(x, y),
      (
        Some(light(&palette.primary.ink)),
        Some(light(&palette.primary.surface))
      ),
      "{frame}"
    );

    variant.set(SimpleVariant::Destructive);
    settle();

    assert_eq!(
      colors_at(x, y),
      (
        Some(light(&palette.destructive.ink)),
        Some(light(&palette.destructive.surface))
      ),
      "re-inked in the destructive look"
    );

    user::hover(button);
    settle();

    assert_eq!(
      background_at(x, y),
      Some(light(&palette.destructive.hover)),
      "the hovered surface is the variant's"
    );

    variant.set(SimpleVariant::Primary);
    settle();

    assert_eq!(
      background_at(x, y),
      Some(light(&palette.primary.hover)),
      "the hovered surface follows a flip"
    );

    testing::trigger_mouse_move(WIDTH - 1, HEIGHT - 1);
    variant.set(SimpleVariant::Destructive);
    testing::trigger_key("Tab");
    settle();

    assert_eq!(testing::focused_node(), Some(button));
    assert_eq!(
      background_at(x, y),
      Some(light(&palette.destructive.focus)),
      "the focused surface is the variant's"
    );

    variant.set(SimpleVariant::Primary);
    settle();

    assert_eq!(
      background_at(x, y),
      Some(light(&palette.primary.focus)),
      "the focused surface follows a flip"
    );
    assert_eq!(
      testing::find_by_role(Role::Button, "Go"),
      Some(button),
      "re-inked, not rebuilt"
    );
  });
}

/// A disabled button takes the palette's disabled surface and ink whatever its variant,
/// hover changing nothing; enabled again, it returns to its look.
#[test]
fn a_disabled_button_takes_the_disabled_surface_and_ink() {
  with_platform(|| {
    let palette = SimplePalette::default();
    let disabled = Signal::new(true);

    mount_themed(move || SimpleButton::outline("Save").derive_disabled(move || disabled.get()));

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (x, y) = locate(&frame, "Save");

    assert_eq!(
      colors_at(x, y),
      (
        Some(light(&palette.disabled_ink)),
        Some(light(&palette.disabled_surface))
      ),
      "{frame}"
    );

    testing::trigger_mouse_move(x, y);
    settle();

    assert_eq!(
      background_at(x, y),
      Some(light(&palette.disabled_surface)),
      "hover changes nothing"
    );

    disabled.set(false);
    settle();

    assert_eq!(
      colors_at(x, y),
      (
        Some(light(&palette.outline.ink)),
        Some(light(&palette.outline.hover))
      ),
      "enabled again, the hovered look"
    );
  });
}

/// A preset below no theme is inked in its palette's default: a filled button on its
/// look's surface in its ink, and an outlined one in its ink on the ambient background.
#[test]
fn a_preset_below_no_theme_is_inked_in_the_palettes_default() {
  with_platform(|| {
    let palette = SimplePalette::default();

    mount_bare(|| {
      Column::new().set_children((SimpleButton::new("OK"), SimpleButton::outline("Back")))
    });

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (ok_x, ok_y) = locate(&frame, "OK");
    let (back_x, back_y) = locate(&frame, "Back");

    assert_eq!(
      colors_at(ok_x, ok_y),
      (
        Some(light(&palette.primary.ink)),
        Some(light(&palette.primary.surface))
      ),
      "{frame}"
    );
    assert_eq!(
      ink_at(back_x, back_y),
      Some(light(&palette.outline.ink)),
      "{frame}"
    );
    assert_ne!(
      background_at(back_x, back_y),
      Some(light(&palette.primary.surface)),
      "the outline's surface is transparent"
    );
  });
}

/// A theme of the palette nested below no theme repaints its own subtree alone, the
/// default answering outside it.
#[test]
fn a_nested_theme_repaints_only_its_subtree() {
  with_platform(|| {
    let palette = SimplePalette::default();
    let red = Color::srgb(1., 0., 0.);
    let white = Color::srgb(1., 1., 1.);

    mount_bare(move || {
      Column::new().set_children((
        SimpleButton::new("Out"),
        Theme::new(SimplePalette::default().set_primary(SimpleLook::new(
          SimpleVariant::Primary,
          red,
          white,
        )))
        .set_child_builder(|| SimpleButton::new("In")),
      ))
    });

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (out_x, out_y) = locate(&frame, "Out");
    let (in_x, in_y) = locate(&frame, "In");

    assert_eq!(
      colors_at(out_x, out_y),
      (
        Some(light(&palette.primary.ink)),
        Some(light(&palette.primary.surface))
      ),
      "the default outside: {frame}"
    );
    assert_eq!(
      colors_at(in_x, in_y),
      (Some(rgb(white)), Some(rgb(red))),
      "the theme's inside: {frame}"
    );
  });
}

/// A derived variant below no theme inks the variant it switches to, the tokens its
/// derived slots name resolving to the palette's default as its fixed ones do.
#[test]
fn a_derived_variant_below_no_theme_inks_the_variant_it_switches_to() {
  with_platform(|| {
    let palette = SimplePalette::default();
    let variant = Signal::new(SimpleVariant::Secondary);

    mount_bare(move || SimpleButton::derive_variant("Go", move || variant.get()));

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (x, y) = locate(&frame, "Go");

    assert_eq!(
      colors_at(x, y),
      (
        Some(light(&palette.secondary.ink)),
        Some(light(&palette.secondary.surface))
      ),
      "{frame}"
    );

    variant.set(SimpleVariant::Destructive);
    settle();

    assert_eq!(
      colors_at(x, y),
      (
        Some(light(&palette.destructive.ink)),
        Some(light(&palette.destructive.surface))
      ),
      "re-inked in the destructive look"
    );
  });
}

/// The number of root definitions.
fn root_tokens() -> usize {
  Scene::read(|scene| scene.root_definitions().len())
}

/// However many presets are mounted, the palette's own definitions are laid at the root
/// once: the root gains exactly the palette's own names it did not define already, its
/// `simple-` tokens, and keeps the terminal's typography where the palette's aliases of
/// it, which apply beneath a theme of the palette alone, would redefine it.
#[test]
fn many_presets_lay_the_palettes_definitions_at_the_root_once() {
  with_platform(|| {
    let expected = Signal::new(0);

    mount_bare(move || {
      let definitions: Vec<(TokenName, Definition<Color>)> = SimplePalette::default().definitions();

      expected.set(Scene::read(|scene| {
        let root = scene.root_definitions();
        let new = definitions
          .iter()
          .filter(|(name, _)| name.as_str().starts_with("simple-") && root.get(*name).is_none())
          .count();

        root.len() + new
      }));

      Column::new().set_children(
        (0..6)
          .map(|index| {
            if index % 2 == 0 {
              SimpleButton::new("Save").into_any()
            }
            else {
              SimpleCard::new().set_child(Text::new("Card")).into_any()
            }
          })
          .collect::<Vec<_>>(),
      )
    });

    let _ = testing::render_to_text(WIDTH, HEIGHT);

    assert!(Scene::read(
      |scene| scene.defines_root_set(TypeId::of::<SimplePalette>())
    ));
    assert_eq!(root_tokens(), expected.get());
    assert_eq!(
      Scene::read(|scene| scene
        .root_definitions()
        .get(typography::INK.name())
        .cloned()),
      Some(TokenValue::Color(
        Typography::default().ink.definition().clone()
      )),
      "the terminal's own ink, not the palette's alias of its foreground"
    );
  });
}

/// A plain text keeps the terminal's own foreground on a page naming no Simple token and
/// beside a preset, whose default reaches the root without its typography aliases; it
/// takes the palette's foreground beneath a theme of the palette, and an app's
/// typography beneath the app's theme beside a preset.
#[test]
fn a_presets_default_leaves_the_plain_text_beside_it() {
  with_platform(|| {
    mount_bare(|| Text::new("plain"));

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (x, y) = locate(&frame, "plain");

    assert_eq!(ink_at(x, y), None, "the terminal's own foreground: {frame}");
  });

  with_platform(|| {
    mount_bare(|| Column::new().set_children((SimpleButton::new("OK"), Text::new("plain"))));

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (x, y) = locate(&frame, "plain");

    assert_eq!(
      ink_at(x, y),
      None,
      "a preset's default at the root leaves the typography alone: {frame}"
    );
  });

  with_platform(|| {
    let palette = SimplePalette::default();

    mount_themed(|| Column::new().set_children((SimpleButton::new("OK"), Text::new("plain"))));

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (x, y) = locate(&frame, "plain");

    assert_eq!(
      ink_at(x, y),
      Some(light(&palette.foreground)),
      "the palette's foreground beneath its theme: {frame}"
    );
  });

  with_platform(|| {
    let red = Color::srgb(1., 0., 0.);

    mount_bare(move || {
      Theme::new(Typography::default().set_ink(red)).set_child_builder(|| {
        Column::new().set_children((SimpleButton::new("OK"), Text::new("plain")))
      })
    });

    let frame = testing::render_to_text(WIDTH, HEIGHT);
    let (x, y) = locate(&frame, "plain");

    assert_eq!(
      ink_at(x, y),
      Some(rgb(red)),
      "the app's typography beside a preset: {frame}"
    );
  });
}
