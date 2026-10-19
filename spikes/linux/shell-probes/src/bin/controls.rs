//! Every drawn control, each change printed on standard error as `[app] <what> <value>` for a
//! harness to read: a field, a secure field, a field whose handler transforms (to capitals)
//! and one whose handler rejects (anything but digits), an editor, a switch, a checkbox, a
//! radio group, a slider, a picker and a button.
//!
//! `PROBE_FIELDS_ONLY=1` shows the fields alone (for the input method scenarios and the reader
//! probes, which compare a field against GtkEntry). `PROBE_READER=1` shows what the GTK
//! reference window (tools/controls/gtk-ref.py) shows, in its order, for the screen reader's
//! comparison: its title, the Name field, the Wi-Fi switch, the Submit button and its status,
//! the field asking for focus as the window opens, as `GtkWindow` focuses its first entry.

use skene_gui::prelude::*;

fn log(what: &str, value: impl std::fmt::Debug) {
  eprintln!("[app] {what} {value:?}");
}

/// The GTK reference window's contents, drawn by Skene.
fn reader() -> Column {
  let name = Signal::new(String::new());
  let wifi = Signal::new(false);
  let presses = Signal::new(0u32);
  let status = Signal::new(String::new());
  let submit = move || {
    presses.update(|presses| *presses += 1);
    status.set(format!("Submitted \"{}\" ({})", name.get(), presses.get()));
    log("status", status.get());
  };

  Column::new()
    .set_spacing(8)
    .add_child(Text::new("Skene reference").set_font_size(20))
    .add_child(
      TextField::new("Name")
        .set_autofocus(true)
        .derive_text(move || name.get())
        .handle_change(move |text| {
          log("name", &text);
          name.set(text);
        })
        .handle_activate(submit),
    )
    .add_child(
      Toggle::new("Wi-Fi")
        .set_accessible_description("Connects to wireless networks")
        .derive_on(move || wifi.get())
        .handle_change(move |on| {
          wifi.set(on);
          status.set(format!("Wi-Fi {}", if on { "on" } else { "off" }));
          log("status", status.get());
        }),
    )
    .add_child(Button::new("Submit").handle_activate(submit))
    .add_child(Text::derive(move || status.get()))
}

#[skene_gui::main]
fn app() -> impl View {
  if std::env::var_os("PROBE_READER").is_some() {
    return Scroll::new(ScrollAxes::Vertical).set_child(
      Container::new()
        .set_padding(Inset::new(20., 24., 20., 24.))
        .set_alignment(Alignment::new(AxisAlignment::Stretch, AxisAlignment::Start))
        .set_child(reader()),
    );
  }

  let name = Signal::new(String::new());
  let secret = Signal::new(String::new());
  let email = Signal::new(String::new());
  let shout = Signal::new(String::new());
  let digits = Signal::new(String::new());
  let notes = Signal::new(String::new());
  let wifi = Signal::new(false);
  let remember = Signal::new(false);
  let size = Signal::new(Some(1usize));
  let volume = Signal::new(0.5f32);
  let fruit = Signal::new(Some(0usize));
  let presses = Signal::new(0u32);
  let fields_only = std::env::var_os("PROBE_FIELDS_ONLY").is_some();
  let status = NodeHandle::new();
  let mut column = Column::new()
    .set_spacing(10)
    .add_child(Text::new("Controls").set_font_size(20))
    .add_child(
      TextField::new("Name")
        .set_placeholder("Your name")
        .derive_text(move || name.get())
        .handle_change(move |text| {
          log("name", &text);
          name.set(text);
        })
        .handle_activate(move || log("name-activate", name.get())),
    )
    .add_child(
      TextField::new("Email")
        .set_keyboard(Keyboard::Email)
        .derive_text(move || email.get())
        .handle_change(move |text| {
          log("email", &text);
          email.set(text);
        }),
    )
    .add_child(
      TextField::new("Password")
        .set_secure(true)
        .derive_text(move || secret.get())
        .handle_change(move |text| {
          log("password-length", text.chars().count());
          secret.set(text);
        }),
    )
    .add_child(
      TextField::new("Shout")
        .derive_text(move || shout.get())
        .handle_change(move |text| {
          let upper = text.to_uppercase();

          log("shout", &upper);
          shout.set(upper);
        }),
    )
    .add_child(
      TextField::new("Digits")
        .derive_text(move || digits.get())
        .handle_change(move |text| {
          if text.chars().all(|character| character.is_ascii_digit()) {
            log("digits", &text);
            digits.set(text);
          } else {
            log("digits-rejected", &text);
          }
        }),
    )
    .add_child(
      TextEditor::new("Notes")
        .set_min_height(72)
        .derive_text(move || notes.get())
        .handle_change(move |text| {
          log("notes", &text);
          notes.set(text);
        }),
    );

  if !fields_only {
    column = column
      .add_child(
        Toggle::new("Wi-Fi")
          .set_accessible_description("Connects to wireless networks")
          .derive_on(move || wifi.get())
          .handle_change(move |on| {
            log("wifi", on);
            wifi.set(on);
          }),
      )
      .add_child(
        Checkbox::new("Remember me")
          .derive_checked(move || remember.get())
          .handle_change(move |on| {
            log("remember", on);
            remember.set(on);
          }),
      )
      .add_child(
        RadioGroup::new("Size")
          .set_options(["Small", "Medium", "Large"])
          .derive_chosen(move || size.get())
          .handle_choose(move |chosen| {
            log("size", chosen);
            size.set(chosen);
          }),
      )
      .add_child(
        Slider::new("Volume")
          .set_range(0., 1.)
          .set_step(0.1)
          .derive_value(move || volume.get())
          .handle_change(move |value| {
            log("volume", value);
            volume.set(value);
          }),
      )
      .add_child(
        Picker::new("Fruit")
          .set_options(["Apple", "Banana", "Blueberry", "Cherry", "Grape"])
          .derive_chosen(move || fruit.get())
          .handle_choose(move |chosen| {
            log("fruit", chosen);
            fruit.set(chosen);
          }),
      )
      .add_child(
        Button::new("Submit")
          .set_prominent(true)
          .set_described_by(&status)
          .handle_activate(move || {
            presses.update(|presses| *presses += 1);
            log("submit", presses.get());
          }),
      )
      .add_child(
        Text::derive(move || format!("Submitted {} times", presses.get())).set_handle(&status),
      );
  }

  Scroll::new(ScrollAxes::Vertical).set_child(
    Container::new()
      .set_padding(16)
      .set_alignment(Alignment::new(AxisAlignment::Stretch, AxisAlignment::Start))
      .set_child(column),
  )
}
