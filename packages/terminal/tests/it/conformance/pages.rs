//! The suite's pages in skene-terminal's vocabulary: a page of each name the suite lists
//! ([`PAGES`](skene_conformance::PAGES)), holding what its scenarios find there.

use std::fmt::{self, Debug, Formatter};

use skene_conformance::{BREAKPOINT, Probes, RETURN};
use skene_terminal::{
  Activate,
  ListboxOption,
  Radio,
  RadioGroup,
  Slider,
  TextEntry,
  Toggle,
  prelude::*,
  schedule,
};

/// A page of the suite: the view a scenario opens, by the name it opens it by.
pub(super) struct Page {
  /// The name a scenario opens it by.
  pub(super) name: &'static str,
  /// Builds the view, publishing into the probes what a scenario checks of it.
  pub(super) build: fn(&mut Probes) -> AnyView,
}

impl Debug for Page {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Page")
      .field("name", &self.name)
      .finish_non_exhaustive()
  }
}

/// Every page of the suite, in the order of [`PAGES`](skene_conformance::PAGES).
#[must_use]
pub(super) fn pages() -> Vec<Page> {
  vec![
    Page {
      name: "button",
      build: button_page,
    },
    Page {
      name: "checkbox",
      build: checkbox_page,
    },
    Page {
      name: "buttons",
      build: buttons_page,
    },
    Page {
      name: "disabled-button",
      build: disabled_button_page,
    },
    Page {
      name: "text-field",
      build: text_field_page,
    },
    Page {
      name: "read-only-field",
      build: read_only_field_page,
    },
    Page {
      name: "radio-group",
      build: radio_group_page,
    },
    Page {
      name: "slider",
      build: slider_page,
    },
    Page {
      name: "picker",
      build: picker_page,
    },
    Page {
      name: "tooltip",
      build: tooltip_page,
    },
    Page {
      name: "context-menu",
      build: context_menu_page,
    },
    Page {
      name: "viewport",
      build: viewport_page,
    },
    Page {
      name: "visibility",
      build: visibility_page,
    },
    Page {
      name: "modal",
      build: modal_page,
    },
    Page {
      name: "form",
      build: form_page,
    },
    Page {
      name: "nested-boxes",
      build: nested_boxes_page,
    },
    Page {
      name: "focus-group",
      build: focus_group_page,
    },
    Page {
      name: "row",
      build: row_page,
    },
    Page {
      name: "autofocus",
      build: autofocus_page,
    },
    Page {
      name: "text",
      build: text_page,
    },
  ]
}

/// The page of the suite named `name`; `None` where none is.
#[must_use]
pub(super) fn page(name: &str) -> Option<Page> {
  pages().into_iter().find(|page| page.name == name)
}

/// A button named `name`.
fn button(name: &str) -> BasicButton {
  BasicButton::new(Activate::new()).set_label(name)
}

/// A button named Save, counting its activations as `clicks`.
fn button_page(probes: &mut Probes) -> AnyView {
  let clicks = Signal::new(0u32);

  probes.publish("clicks", move || clicks.get().to_string());

  button("Save")
    .handle_activate(move || clicks.update(|count| *count += 1))
    .into_any()
}

/// A checkbox named Remember me, bound to `checked`.
fn checkbox_page(probes: &mut Probes) -> AnyView {
  let checked = Signal::new(false);

  probes.publish("checked", move || checked.get().to_string());

  BasicCheckbox::new(Toggle::new().bind_checked(checked))
    .set_label("Remember me")
    .into_any()
}

/// The buttons One, Two and Three in a column.
fn buttons_page(_: &mut Probes) -> AnyView {
  Column::new()
    .set_children((button("One"), button("Two"), button("Three")))
    .into_any()
}

/// The buttons One, Two and Three in a column, Two disabled and counting its activations
/// as `clicks`.
fn disabled_button_page(probes: &mut Probes) -> AnyView {
  let clicks = Signal::new(0u32);

  probes.publish("clicks", move || clicks.get().to_string());

  Column::new()
    .set_children((
      button("One"),
      button("Two")
        .set_disabled(true)
        .handle_activate(move || clicks.update(|count| *count += 1)),
      button("Three"),
    ))
    .into_any()
}

/// A text field named Name, its text bound to `value` and its selection to `selection`
/// as `start..end`, above the plain text Plain words.
fn text_field_page(probes: &mut Probes) -> AnyView {
  let value = Signal::new(String::new());
  let selection = Signal::new(TextSelection::collapsed(0));

  probes.publish("value", move || value.get());
  probes.publish("selection", move || {
    let selection = selection.get();

    format!("{}..{}", selection.start, selection.end)
  });

  Column::new()
    .set_children((
      BasicTextField::new(TextEntry::new().bind_value(value).bind_selection(selection))
        .set_label("Name"),
      Text::new("Plain words"),
    ))
    .into_any()
}

/// A read-only text field named Name holding fixed, bound to `value`.
fn read_only_field_page(probes: &mut Probes) -> AnyView {
  let value = Signal::new("fixed".to_owned());

  probes.publish("value", move || value.get());

  Column::new()
    .set_children((BasicTextField::new(TextEntry::new().bind_value(value))
      .set_label("Name")
      .set_read_only(true),))
    .into_any()
}

/// The size a radio group chooses.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Fit {
  Small,
  Medium,
  Large,
}

impl Fit {
  /// The probe's text for the size.
  const fn probe(self) -> &'static str {
    match self {
      Self::Small => "s",
      Self::Medium => "m",
      Self::Large => "l",
    }
  }
}

/// A letter a picker chooses.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Letter {
  Alpha,
  Bravo,
  Charlie,
}

impl Letter {
  /// The letter's name, which is also the probe's text for it.
  const fn name(self) -> &'static str {
    match self {
      Self::Alpha => "a",
      Self::Bravo => "b",
      Self::Charlie => "c",
    }
  }
}

/// A radio group named Size of Small, Medium and Large, its choice bound to `chosen`
/// (empty while nothing is chosen), above a button named Next.
fn radio_group_page(probes: &mut Probes) -> AnyView {
  let chosen: Signal<Option<Fit>> = Signal::new(None);

  probes.publish("chosen", move || {
    chosen.get().map(Fit::probe).unwrap_or_default().to_owned()
  });

  Column::new()
    .set_children((
      Control::new(RadioGroup::new().bind_chosen(chosen))
        .set_accessible_name("Size")
        .set_child(Column::new().set_children((
          BasicRadio::new(Radio::new(Fit::Small)).set_label("Small"),
          BasicRadio::new(Radio::new(Fit::Medium)).set_label("Medium"),
          BasicRadio::new(Radio::new(Fit::Large)).set_label("Large"),
        ))),
      button("Next"),
    ))
    .into_any()
}

/// A slider named Volume from 0 to 100, its value bound to `value`.
fn slider_page(probes: &mut Probes) -> AnyView {
  let value = Signal::new(50f32);

  probes.publish("value", move || value.get().to_string());

  BasicSlider::new(Slider::new().bind_value(value))
    .set_accessible_name("Volume")
    .into_any()
}

/// A picker named Letter of a, b and c, its choice bound to `chosen`, above a button
/// named Elsewhere.
fn picker_page(probes: &mut Probes) -> AnyView {
  let chosen = Signal::new(Letter::Bravo);

  probes.publish("chosen", move || chosen.get().name().to_owned());

  Column::new()
    .set_children((
      BasicPicker::new()
        .bind_chosen(chosen)
        .set_label("Letter")
        .set_options(
          [Letter::Alpha, Letter::Bravo, Letter::Charlie].map(|letter| {
            BasicListboxOption::new(ListboxOption::new(letter)).set_label(letter.name())
          }),
        )
        .set_face(|shown| {
          Text::derive(move || shown.get().map(Letter::name).unwrap_or_default().to_owned())
        }),
      button("Elsewhere"),
    ))
    .into_any()
}

/// A button named Save, described by the tooltip Saves your work.
fn tooltip_page(_: &mut Probes) -> AnyView {
  Tooltip::new("Saves your work")
    .set_child(button("Save"))
    .into_any()
}

/// A focusable group named Note serving a context menu of one entry, Rename, whose
/// activation sets `renamed`.
fn context_menu_page(probes: &mut Probes) -> AnyView {
  let renamed = Signal::new(false);

  probes.publish("renamed", move || renamed.get().to_string());

  ContextMenu::new()
    .set_items([MenuEntry::new("Rename").handle_activate(move || renamed.set(true))])
    .set_child(
      Container::new()
        .set_role(Role::Group)
        .set_label("Note")
        .set_focus_order(FocusOrder::Auto)
        .set_child(Text::new("A note")),
    )
    .into_any()
}

/// The text Wide, hidden below the [`BREAKPOINT`] width.
fn viewport_page(_: &mut Probes) -> AnyView {
  Container::new()
    .set_hidden(true.when(Media::min_width(BREAKPOINT), false))
    .set_child(Text::new("Wide"))
    .into_any()
}

/// A button named Dismiss, whose activation hides it for [`RETURN`], above a button
/// named Elsewhere.
fn visibility_page(_: &mut Probes) -> AnyView {
  let shown = Signal::new(true);

  Column::new()
    .set_children((
      Visibility::new()
        .derive_shown(move || shown.get())
        .set_child(button("Dismiss").handle_activate(move || {
          shown.set(false);

          let _timer = schedule(RETURN, move || shown.set(true));
        })),
      button("Elsewhere"),
    ))
    .into_any()
}

/// A button named Open, which opens a modal holding a button named Inside, `open` as
/// the modal is open.
fn modal_page(probes: &mut Probes) -> AnyView {
  let open = Signal::new(false);

  probes.publish("open", move || open.get().to_string());

  Column::new()
    .set_children((
      button("Open").handle_activate(move || open.set(true)),
      Overlay::modal().bind_open(open).set_child(button("Inside")),
    ))
    .into_any()
}

/// A sign-in form: a heading, a text field holding ada, a checked checkbox and a
/// disabled button, none bound.
fn form_page(_: &mut Probes) -> AnyView {
  Column::new()
    .set_children((
      Container::new()
        .set_role(Role::Heading)
        .set_child(Text::new("Sign in")),
      BasicTextField::new(TextEntry::new().set_value("ada")).set_label("Name"),
      BasicCheckbox::new(Toggle::new().set_checked(true)).set_label("Remember me"),
      button("Continue").set_disabled(true),
    ))
    .into_any()
}

/// A box named Outer around a box named Inner, neither focusable.
fn nested_boxes_page(_: &mut Probes) -> AnyView {
  Container::new()
    .set_role(Role::Group)
    .set_label("Outer")
    .set_padding(Inset::all(4))
    .set_child(
      Container::new()
        .set_role(Role::Group)
        .set_label("Inner")
        .set_width(40)
        .set_height(8),
    )
    .into_any()
}

/// A box around the buttons a and b, counting focus entering it as `entered` and
/// leaving it as `left`, above a button named c.
fn focus_group_page(probes: &mut Probes) -> AnyView {
  let entered = Signal::new(0u32);
  let left = Signal::new(0u32);

  probes.publish("entered", move || entered.get().to_string());
  probes.publish("left", move || left.get().to_string());

  Column::new()
    .set_children((
      Container::new()
        .handle_focus(move || entered.update(|count| *count += 1))
        .handle_blur(move || left.update(|count| *count += 1))
        .set_child(Row::new().set_children((button("a"), button("b")))),
      button("c"),
    ))
    .into_any()
}

/// An activatable row named Row holding a button named Inside, publishing the row's
/// activations as `row` and the button's as `inside`.
fn row_page(probes: &mut Probes) -> AnyView {
  let row = Signal::new(0u32);
  let inside = Signal::new(0u32);

  probes.publish("row", move || row.get().to_string());
  probes.publish("inside", move || inside.get().to_string());

  Container::new()
    .set_role(Role::Group)
    .set_label("Row")
    .handle_click(move || row.update(|count| *count += 1))
    .set_child(button("Inside").handle_activate(move || inside.update(|count| *count += 1)))
    .into_any()
}

/// The buttons a and b in a column, b set to autofocus.
fn autofocus_page(_: &mut Probes) -> AnyView {
  Column::new()
    .set_children((button("a"), button("b").set_autofocus(true)))
    .into_any()
}

/// Text keeping its blanks and newlines: two spaces between words, two lines, a line
/// starting with blanks, and a word too wide for the page broken where the page's width
/// ends.
fn text_page(_: &mut Probes) -> AnyView {
  Column::new()
    .set_spacing(16)
    .set_children((
      Text::new("Two  spaces between words"),
      Text::new("One line\nand the next"),
      Text::new("    indented"),
      Text::new("Unbreakable".repeat(8)),
    ))
    .into_any()
}
