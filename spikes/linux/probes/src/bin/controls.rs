//! The controls on real GTK, acted on with real X input: each reports the reader's change
//! with its own timing, is answered with the truth, and shows it; writes from a signal report
//! nothing; a disabled control takes no input or focus; and what a screen reader finds over
//! AT-SPI (names, captions, descriptions, required and invalid, checked and value) follows the
//! signals.

use std::{cell::Cell, rc::Rc, time::Duration};

use probes::{atspi, key, line, result, run, settle, show, wait, widget};
use skene_gui::{
  platform::linux::{
    CheckButton,
    gtk::{self, prelude::*},
  },
  prelude::*,
};

/// Runs the loop until `holds` or a second passes.
fn eventually(holds: impl Fn() -> bool) -> bool {
  let start = std::time::Instant::now();

  while !holds() {
    if start.elapsed() > Duration::from_secs(1) {
      return false;
    }

    wait(20);
  }

  true
}

fn counter() -> (Rc<Cell<u32>>, impl Fn() -> u32) {
  let count = Rc::new(Cell::new(0));
  let read = Rc::clone(&count);

  (count, move || read.get())
}

fn body() {
  // A switch: a flip the handler rejects is vetoed; one it accepts shows; a write from the
  // signal reports nothing.
  let (wifi, locked) = (Signal::new(false), Signal::new(true));
  let (flips, flipped) = counter();
  let [switch, check, check_rejected, mixed] = std::array::from_fn(|_| NodeHandle::new());
  let remember = Signal::new(false);
  let (checks, checked) = counter();
  let shown = show(400, -1, || {
    Column::new()
      .set_padding(10)
      .set_spacing(10)
      .add_child(
        Toggle::new("Wi-Fi")
          .derive_on(move || wifi.get())
          .handle_change(move |on| {
            flips.set(flips.get() + 1);

            if !locked.get() {
              wifi.set(on);
            }
          })
          .set_handle(&switch),
      )
      .add_child(
        CheckButton::new()
          .set_caption("Remember me")
          .bind_on(remember)
          .set_handle(&check),
      )
      .add_child(
        CheckButton::new()
          .set_caption("Never")
          .handle_change(move |_| checks.set(checks.get() + 1))
          .set_handle(&check_rejected),
      )
      .add_child(
        CheckButton::new()
          .set_caption("Some")
          .derive_mixed(|| true)
          .set_handle(&mixed),
      )
  });
  let gtk_switch = widget(&switch)
    .downcast::<gtk::Switch>()
    .expect("a GtkSwitch");

  shown.click(gtk_switch.upcast_ref());

  result(
    "switch: a flip the handler rejects is vetoed",
    flipped() == 1 && !wifi.get() && !gtk_switch.is_active() && !gtk_switch.state(),
    format!(
      "reports {}, signal {}, active {}, state {}",
      flipped(),
      wifi.get(),
      gtk_switch.is_active(),
      gtk_switch.state()
    ),
  );

  locked.set(false);
  shown.click(gtk_switch.upcast_ref());

  result(
    "switch: a flip the handler accepts shows",
    flipped() == 2 && wifi.get() && gtk_switch.is_active() && gtk_switch.state(),
    format!("signal {}, active {}", wifi.get(), gtk_switch.is_active()),
  );

  wifi.set(false);
  settle();

  result(
    "switch: a write from the signal shows and reports nothing",
    flipped() == 2 && !gtk_switch.is_active(),
    format!("reports {}, active {}", flipped(), gtk_switch.is_active()),
  );

  let gtk_check = widget(&check)
    .downcast::<gtk::CheckButton>()
    .expect("a GtkCheckButton");

  shown.click(gtk_check.upcast_ref());

  let rejected = widget(&check_rejected)
    .downcast::<gtk::CheckButton>()
    .expect("a GtkCheckButton");

  shown.click(rejected.upcast_ref());

  result(
    "check button: an accepted check shows, a rejected one is undone at once",
    remember.get() && gtk_check.is_active() && checked() == 1 && !rejected.is_active(),
    format!(
      "accepted: signal {} active {}; rejected: reports {} active {}",
      remember.get(),
      gtk_check.is_active(),
      checked(),
      rejected.is_active()
    ),
  );

  let mixed = widget(&mixed)
    .downcast::<gtk::CheckButton>()
    .expect("a GtkCheckButton");

  result(
    "check button: derive_mixed is GTK's inconsistent",
    mixed.is_inconsistent(),
    mixed.is_inconsistent(),
  );

  shown.close();

  // A slider clamps through change-value: End moves it to its range's end, the handler holds
  // it at 50, and the scale shows 50.
  let volume = Signal::new(10_f32);
  let [slider] = std::array::from_fn(|_| NodeHandle::new());
  let shown = show(400, -1, || {
    Slider::new("Volume")
      .set_range(0., 100.)
      .set_step(1.)
      .derive_value(move || volume.get())
      .handle_change(move |moved| volume.set(moved.min(50.)))
      .set_handle(&slider)
  });
  let scale = widget(&slider)
    .downcast::<gtk::Scale>()
    .expect("a GtkScale");

  scale.grab_focus();
  key("End");

  result(
    "slider: End proposes 100, the handler holds it at 50, the scale shows 50",
    volume.get() == 50. && scale.value() == 50.,
    format!("signal {}, scale {}", volume.get(), scale.value()),
  );

  key("Left");

  result(
    "slider: Left steps by 1",
    volume.get() == 49. && scale.value() == 49.,
    format!("signal {}, scale {}", volume.get(), scale.value()),
  );

  shown.close();

  // A button activates on a click and from the keyboard.
  let (presses, pressed) = counter();
  let enabled = Signal::new(false);
  let [button, disabled] = std::array::from_fn(|_| NodeHandle::new());
  let (disabled_presses, disabled_pressed) = counter();
  let shown = show(400, -1, || {
    Column::new()
      .set_spacing(10)
      .add_child(
        Button::new("Save")
          .handle_activate(move || presses.set(presses.get() + 1))
          .set_handle(&button),
      )
      .add_child(
        Button::new("Later")
          .derive_enabled(move || enabled.get())
          .handle_activate(move || disabled_presses.set(disabled_presses.get() + 1))
          .set_handle(&disabled),
      )
  });

  shown.click(&widget(&button));

  let clicked = pressed();

  key("space");

  let spaced = eventually(|| pressed() == 2);

  result(
    "button: a click and Space each activate it once",
    clicked == 1 && spaced,
    format!("after the click {clicked}, after Space {}", pressed()),
  );

  shown.click(&widget(&disabled));

  let gtk_disabled = widget(&disabled);

  key("Tab");

  let focus_skips = !gtk_disabled.has_focus();

  result(
    "a disabled control takes no click and no focus",
    disabled_pressed() == 0 && !gtk_disabled.is_sensitive() && focus_skips,
    format!(
      "activations {}, sensitive {}, focused after Tab {}",
      disabled_pressed(),
      gtk_disabled.is_sensitive(),
      gtk_disabled.has_focus()
    ),
  );

  enabled.set(true);
  settle();
  shown.click(&gtk_disabled);

  result(
    "derive_enabled(true) gives it back",
    disabled_pressed() == 1 && gtk_disabled.is_sensitive(),
    format!("activations {}", disabled_pressed()),
  );

  shown.close();

  // A picker: the reader's choice through GTK's own popup, accepted; and one the handler
  // ignores, undone.
  let size = Signal::new(Some(0));
  let (choices, chosen) = counter();
  let [picker, stubborn] = std::array::from_fn(|_| NodeHandle::new());
  let shown = show(400, -1, || {
    Column::new()
      .set_spacing(10)
      .add_child(
        Picker::new("Size")
          .set_options(["Small", "Medium", "Large"])
          .bind_chosen(size)
          .set_handle(&picker),
      )
      .add_child(
        Picker::new("Fixed")
          .set_options(["One", "Two", "Three"])
          .derive_chosen(|| Some(0))
          .handle_choose(move |_| choices.set(choices.get() + 1))
          .set_handle(&stubborn),
      )
  });
  let dropdown = widget(&picker)
    .downcast::<gtk::DropDown>()
    .expect("a GtkDropDown");

  shown.click(dropdown.upcast_ref());
  wait(300);
  key("Down");
  key("Down");
  key("Return");
  wait(100);
  settle();

  result(
    "picker: Down and Return in GTK's popup choose Large",
    size.get() == Some(2) && dropdown.selected() == 2,
    format!("signal {:?}, shown {}", size.get(), dropdown.selected()),
  );

  let fixed = widget(&stubborn)
    .downcast::<gtk::DropDown>()
    .expect("a GtkDropDown");

  shown.click(fixed.upcast_ref());
  wait(300);
  key("Down");
  key("Return");
  wait(100);
  settle();

  result(
    "picker: a choice the handler ignores is undone",
    chosen() == 1 && fixed.selected() == 0,
    format!("reports {}, shown {}", chosen(), fixed.selected()),
  );

  shown.close();

  // A box hears the keys its controls leave and the presses outside them.
  let keys = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
  let presses = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
  let [field, pad] = [NodeHandle::new(), NodeHandle::new()];
  let shown = {
    let (keys, presses) = (Rc::clone(&keys), Rc::clone(&presses));

    show(400, -1, move || {
      Container::new()
        .handle_key(move |event| keys.borrow_mut().push(event.key.clone()))
        .handle_pointer(move |event| presses.borrow_mut().push(format!("{:?}", event.phase)))
        .set_child(
          Column::new()
            .set_spacing(10)
            .add_child(TextField::new("Typing").set_handle(&field))
            .add_child(Container::new().set_height(40).set_handle(&pad)),
        )
    })
  };

  shown.click(&widget(&field));
  probes::type_text("ab");
  key("Escape");
  shown.click(&widget(&pad));

  result(
    "a box hears the keys its controls leave (Escape, not letters) and presses outside them",
    *keys.borrow() == ["Escape"]
      && presses
        .borrow()
        .ends_with(&["Down".to_owned(), "Up".to_owned()]),
    format!("keys {:?}, presses {:?}", keys.borrow(), presses.borrow()),
  );

  shown.close();

  // What a screen reader finds, and that it follows the signals.
  let (on, level, invalid) = (Signal::new(false), Signal::new(30_f32), Signal::new(true));
  let hint = NodeHandle::new();
  let shown = show(400, -1, || {
    Container::new().set_label("Preferences").set_child(
      Column::new()
        .set_spacing(8)
        .add_child(Toggle::new("Bluetooth").bind_on(on))
        .add_child(
          Slider::new("Brightness")
            .set_range(0., 100.)
            .bind_value(level),
        )
        .add_child(
          TextField::new("Code")
            .set_required(true)
            .derive_invalid(move || invalid.get())
            .set_described_by(&hint),
        )
        .add_child(Text::new("Six digits").set_handle(&hint))
        .add_child(TextField::new("Serial").set_text("A-1").set_read_only(true))
        .add_child(
          Button::new("Go")
            .set_accessible_name("Start the transfer")
            .set_accessible_description("Sends everything now"),
        ),
    )
  });
  let before = atspi();

  on.set(true);
  level.set(70.);
  invalid.set(false);
  settle();

  let after = atspi();
  let found = |dump: &[String], parts: &[&str]| line(dump, parts).unwrap_or("(none)").to_owned();
  let switch_before = found(&before, &["switch", "\"Bluetooth\""]);
  let switch_after = found(&after, &["switch", "\"Bluetooth\""]);
  let slider_before = found(&before, &["slider", "\"Brightness\""]);
  let slider_after = found(&after, &["slider", "\"Brightness\""]);
  let code_before = found(&before, &["text box", "\"Code\""]);
  let code_after = found(&after, &["text box", "\"Code\""]);
  let go = found(&after, &["button", "\"Start the transfer\""]);
  let serial = found(&after, &["text box", "\"Serial\""]);
  let region = found(&after, &["\"Preferences\""]);

  for line in &after {
    println!("ATSPI {line}");
  }

  result(
    "a switch named by its caption, its checked state following the signal",
    switch_before.contains("labelled-by=\"Bluetooth\"")
      && !switch_before.contains("checked")
      && switch_after.contains("checked"),
    format!("before: {switch_before} | after: {switch_after}"),
  );
  result(
    "a slider's value follows the signal",
    slider_before.contains("value=30") && slider_after.contains("value=70"),
    format!("before: {slider_before} | after: {slider_after}"),
  );
  result(
    "a field: required, invalid while the signal says, described by a text",
    code_before.contains("required")
      && code_before.contains("invalid-entry")
      && code_before.contains("described-by=\"Six digits\"")
      && !code_after.contains("invalid-entry"),
    format!("before: {code_before} | after: {code_after}"),
  );
  result(
    "a button named and described by text",
    go.contains("desc=\"Sends everything now\""),
    go,
  );
  result(
    "a read-only field is read-only to a reader, its text still read",
    serial.contains("read-only") && !serial.contains("editable") && serial.contains("A-1"),
    serial,
  );
  result(
    "a labelled box is a named region",
    region.contains("landmark") || region.contains("region"),
    region,
  );

  shown.close();
}

fn main() -> std::process::ExitCode {
  run("controls", body)
}
