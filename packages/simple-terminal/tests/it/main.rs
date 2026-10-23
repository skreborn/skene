//! The Simple library on the terminal: what the browser's scripted input cannot give,
//! the pointer resting on a control and the keys walking a picker, read as the cells the
//! terminal paints.

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use skene_simple_terminal::prelude::*;
use skene_terminal::{
  self as terminal,
  StaticLeaf,
  Token,
  prelude::*,
  testing::{self, locate, user, with_platform},
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

  let _ = terminal::mount(|| {
    Theme::new(SimplePalette::default())
      .set_child_builder(view)
      .build()
  });
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
    let value = Signal::new("md".to_owned());

    mount_themed(move || {
      Column::new().set_children((SimplePicker::new()
        .set_options([("sm", "Small"), ("md", "Medium"), ("lg", "Large")])
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

/// A disabled button takes the palette's disabled surface and ink whatever its variant,
/// hover changing nothing; enabled again, it returns to its look.
#[test]
fn a_disabled_button_takes_the_disabled_surface_and_ink() {
  with_platform(|| {
    let palette = SimplePalette::default();
    let disabled = Signal::new(true);

    mount_themed(move || {
      SimpleButton::new("Save")
        .set_variant(SimpleVariant::Outline)
        .derive_disabled(move || disabled.get())
    });

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
