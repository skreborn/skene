//! The built-in controls as a reader and the screen find them: a control takes presses inside
//! its own box alone, a slider is as wide as it says, a caption is never required, a
//! library's text entry takes the standard context menu as a built-in's does, and a disabled
//! control's caption dims with it wherever it sits.

use skene_gui::{
  BasicTextField,
  behavior::TextEntry,
  prelude::*,
  testing::{self, Role, Screenshot},
};

/// The darkest sum of a pixel's channels inside `bounds` of `shot`: lower is darker ink.
fn darkest(shot: &Screenshot, bounds: skene_gui::Rect) -> u32 {
  let mut least = u32::MAX;
  let mut y = bounds.y.ceil();

  while y < bounds.y + bounds.height {
    let mut x = bounds.x.ceil();

    while x < bounds.x + bounds.width {
      let [red, green, blue, _] = shot.rgba_at(x, y);

      least = least.min(u32::from(red) + u32::from(green) + u32::from(blue));
      x += 1.;
    }

    y += 1.;
  }

  least
}

/// Two sliders in rows, one at the middle of its range and one at its start.
fn sliders() -> impl View {
  let half = Signal::new(0.5_f32);
  let zero = Signal::new(0.0_f32);

  Column::new()
    .set_padding(16)
    .set_spacing(24)
    .add_child(Row::new().add_child(Slider::new("Half").bind_value(half)))
    .add_child(Row::new().add_child(Slider::new("Zero").bind_value(zero)))
}

#[skene_gui::test]
fn a_slider_takes_a_press_inside_its_own_box_alone() {
  let screen = testing::render(sliders);
  let half = screen.find_by_role(Role::Slider, "Half");
  let zero = screen.find_by_role(Role::Slider, "Zero");

  half.click_at(1.25, 0.5);
  assert!(
    (half.value() - 0.5).abs() < 1e-6,
    "a press past the slider's trailing edge leaves it: {}",
    half.value()
  );
  zero.click_at(1.25, 0.5);
  assert!(zero.value().abs() < 1e-6, "{}", zero.value());
  half.click_at(0.9, 0.5);
  assert!(
    half.value() > 0.75,
    "a press inside the slider still moves it: {}",
    half.value()
  );
}

/// A slider of each kind of width: natural in a row, set, and stretched by a column.
fn widths() -> impl View {
  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Row::new().add_child(Slider::new("Natural")))
    .add_child(Row::new().add_child(Slider::new("Set").set_width(240)))
    .add_child(Column::new().add_child(Slider::new("Stretched")))
}

#[skene_gui::test]
fn a_slider_is_200_wide_unless_its_layout_stretches_it_or_its_width_is_set() {
  let screen = testing::render(widths);
  let width = |name| screen.find_by_role(Role::Slider, name).read().bounds.width;

  assert!(
    (width("Natural") - 200.).abs() < 0.5,
    "{}",
    width("Natural")
  );
  assert!((width("Set") - 240.).abs() < 0.5, "{}", width("Set"));
  assert!(
    width("Stretched") > 300.,
    "a column stretches it: {}",
    width("Stretched")
  );
}

/// Controls given no caption, between boxes 10 pixels tall, nothing between them.
fn captionless() -> impl View {
  let wifi = Signal::new(false);

  Column::new()
    .set_padding(0)
    .set_spacing(0)
    .add_child(
      Container::new()
        .set_label("Top")
        .set_width(40)
        .set_height(10),
    )
    .add_child(TextField::new("").set_accessible_name("Search"))
    .add_child(
      Container::new()
        .set_label("Middle")
        .set_width(40)
        .set_height(10),
    )
    .add_child(Slider::new("").set_accessible_name("Level"))
    .add_child(Toggle::new("").set_accessible_name("Wi-Fi").bind_on(wifi))
}

#[skene_gui::test]
fn a_control_given_no_caption_lays_out_none() {
  let screen = testing::render(captionless);
  let top = screen.find_by_role(Role::Region, "Top").read().bounds;
  let field = screen.find_by_role(Role::TextField, "Search").read().bounds;
  let middle = screen.find_by_role(Role::Region, "Middle").read().bounds;
  let level = screen.find_by_role(Role::Slider, "Level").read().bounds;

  assert!(
    (field.y - (top.y + top.height)).abs() < 0.5,
    "the field sits right below the box above it: {top:?} then {field:?}"
  );
  assert!(
    (level.y - (middle.y + middle.height)).abs() < 0.5,
    "the slider sits right below the box above it: {middle:?} then {level:?}"
  );
  assert!(
    screen.find_by_role(Role::Switch, "Wi-Fi").click().is_on(),
    "a captionless switch still flips"
  );
}

/// The built-in field and a library's own text entry with a caption beside it, each in the
/// standard context menu.
fn menus() -> impl View {
  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Built-in"))
    .add_child(
      ContextMenu::standard().set_child(
        BasicTextField::new(TextEntry::new(1))
          .set_min_width(200)
          .set_min_height(30)
          .set_label("Library"),
      ),
    )
}

#[skene_gui::test]
fn a_librarys_text_entry_takes_the_standard_context_menu_as_the_built_in_does() {
  let screen = testing::render(menus);

  for name in ["Built-in", "Library"] {
    let field = screen.find_by_role(Role::TextField, name);

    assert!(field.read().has_popup, "{name} reports its menu");
    field.type_text("abc");
    field.context_click();
    assert!(
      screen.query_by_role(Role::MenuItem, "Copy").is_some(),
      "{name}'s menu opened: {}",
      screen.outline()
    );
    screen.focused().press("Escape");
    assert!(
      screen.query_by_role(Role::MenuItem, "Copy").is_none(),
      "{name}'s menu closed"
    );
  }
}

/// Captions beside their controls and inside them, enabled and disabled.
fn disabled() -> impl View {
  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Enabled field"))
    .add_child(TextField::new("Disabled field").set_disabled(true))
    .add_child(Checkbox::new("Enabled box"))
    .add_child(Checkbox::new("Disabled box").set_disabled(true))
}

#[skene_gui::test]
fn a_disabled_controls_caption_dims_with_it_beside_it_or_inside_it() {
  let screen = testing::render(disabled);
  let shot = screen.render_to_image();
  let ink = |caption: &str| darkest(&shot, screen.find_by_text(caption).read().bounds);
  let (field, disabled_field) = (ink("Enabled field"), ink("Disabled field"));
  let (checkbox, disabled_checkbox) = (ink("Enabled box"), ink("Disabled box"));

  assert!(
    disabled_field > field + 100,
    "a caption beside a disabled field dims: {field} enabled, {disabled_field} disabled"
  );
  assert!(
    disabled_checkbox > checkbox + 100,
    "a caption inside a disabled checkbox dims: {checkbox} enabled, {disabled_checkbox} disabled"
  );
  assert!(
    (i64::from(disabled_field) - i64::from(disabled_checkbox)).abs() < 60,
    "both dim alike: {disabled_field} and {disabled_checkbox}"
  );
}
