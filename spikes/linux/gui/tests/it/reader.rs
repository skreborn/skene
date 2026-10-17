//! What a reader finds when a control names a caption the reader's tree leaves out: the
//! tree stays whole, and the control is named by the caption's text as a browser names a
//! control by a hidden label.

use skene_gui::{
  prelude::*,
  testing::{self, Role},
};

/// A checkbox labelled by a picture a reader skips, its description unset.
fn labelled_by_a_picture() -> impl View {
  let swatch = NodeHandle::new();

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Painted::new(|_| {})
        .set_width(10)
        .set_height(10)
        .set_handle(&swatch),
    )
    .add_child(Checkbox::new("Own caption").set_labelled_by(&swatch))
    .add_child(Button::new("After"))
}

#[skene_gui::test]
fn a_control_labelled_by_a_picture_a_reader_skips_leaves_the_tree_whole() {
  let screen = testing::render(labelled_by_a_picture);
  let after = screen.find_by_role(Role::Button, "After");

  after.press("space");
  assert!(after.is_focused(), "the reader still reads the window");
  assert!(
    screen.outline().contains("After"),
    "the reader reads the whole tree: {}",
    screen.outline()
  );
}

/// A checkbox labelled and described by texts inside an overlay a button closes.
fn labelled_by_an_overlay() -> impl View {
  let caption = NodeHandle::new();
  let hint = NodeHandle::new();
  let open = Signal::new(true);
  let checked = Signal::new(false);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Button::new("Close").handle_activate(move || open.set(false)))
    .add_child(
      Overlay::popup().bind_open(open).set_child(
        Column::new()
          .add_child(Text::new("Hidden caption").set_handle(&caption))
          .add_child(Text::new("Hidden hint").set_handle(&hint)),
      ),
    )
    .add_child(
      Checkbox::new("Own caption")
        .set_labelled_by(&caption)
        .set_described_by(&hint)
        .bind_checked(checked),
    )
}

#[skene_gui::test]
fn a_control_labelled_by_a_caption_that_hides_is_named_by_its_text() {
  let screen = testing::render(labelled_by_an_overlay);
  let checkbox = screen.find_by_role(Role::Checkbox, "Hidden caption");

  assert_eq!(checkbox.description(), "Hidden hint");
  screen.find_by_role(Role::Button, "Close").click();
  assert!(
    screen.query_by_text("Hidden caption").is_none(),
    "the overlay closed: {}",
    screen.outline()
  );

  let checkbox = screen.find_by_role(Role::Checkbox, "Hidden caption");

  assert_eq!(checkbox.description(), "Hidden hint");
  checkbox.click();
  assert!(checkbox.is_on(), "the checkbox still answers");
}

/// A field whose caption the public override places leading.
fn a_leading_caption() -> impl View {
  let text = Signal::new(String::new());

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      TextField::new("Leading field")
        .set_label_placement(LabelPlacement::Leading)
        .bind_text(text),
    )
    .add_child(TextField::new("Trailing field").set_label_placement(LabelPlacement::Trailing))
}

#[skene_gui::test]
fn a_text_fields_leading_or_trailing_caption_sits_beside_it_and_names_it() {
  let screen = testing::render(a_leading_caption);
  let leading = screen.find_by_role(Role::TextField, "Leading field");
  let trailing = screen.find_by_role(Role::TextField, "Trailing field");
  let leading_caption = screen.find_by_text("Leading field").read().bounds;
  let trailing_caption = screen.find_by_text("Trailing field").read().bounds;
  let (leading, trailing) = (leading.read().bounds, trailing.read().bounds);

  assert!(
    leading_caption.y + leading_caption.height <= leading.y + 0.5,
    "a leading caption sits before its field: {leading_caption:?} over {leading:?}"
  );
  assert!(
    trailing.y + trailing.height <= trailing_caption.y + 0.5,
    "a trailing caption sits after its field: {trailing:?} over {trailing_caption:?}"
  );
  screen
    .find_by_role(Role::TextField, "Leading field")
    .type_text("abc");
  assert_eq!(
    screen.find_by_role(Role::TextField, "Leading field").text(),
    "abc"
  );
}
