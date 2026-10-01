# Controls

A control is a view a user operates: a button, a checkbox, a slider, a text field, a
picker. Simple's controls come drawn and ready, in the platform's crate of the Simple
library:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

fn settings(name: Signal<String>, agreed: Signal<bool>) -> impl View {
  Column::new().set_spacing(12.).set_children((
    SimpleTextField::new()
      .set_label("Name")
      .set_placeholder("Your name")
      .bind_value(name),
    SimpleCheckbox::new().set_label("I agree").bind_checked(agreed),
    SimpleButton::new("Save").handle_activate(move || println!("saved {}", name.get())),
  ))
}
```

## The bound signal is the truth

A control reports what its user does, and its signal decides what it shows. `bind_value`
wires both to one signal; `derive_value` and `handle_change` take them apart, so a
handler can refuse a change and the control returns to the signal's value:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

fn digits(code: Signal<String>) -> impl View {
  SimpleTextField::new()
    .set_label("Code")
    .derive_value(move || code.get())
    .handle_change(move |typed: &str| {
      if typed.chars().all(|character| character.is_ascii_digit()) {
        code.set(typed.to_owned());
      }
    })
}
```

## A control owns its events

A control consumes the events it acts on: a press on a button is a press on that button,
never also on the row it sits in. A handler takes a plain callback, and the control
decides what the platform and its ancestors are told. A `Container` is a box rather than
a control, and its handlers choose whether an event travels on, so a group that must
observe its controls' clicks in one place wraps them in one.

## Labels and accessibility

A control's label sits where its kind places it: beside a checkbox's box, before a text
field. A label is a convenience: `set_labelled_by` names a control by a caption placed
anywhere, and `set_accessible_name` by text alone. What a control is (checked, expanded,
its value) is reported by its behavior, and each platform maps it to its own layer: on
the web a form element takes the native attribute, so a form enforces it without
script.

A disabled control takes no focus.

## Groups

A radio group holds its radios as children and binds the chosen one:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

fn sizes(size: Signal<Option<String>>) -> impl View {
  SimpleRadioGroup::new().set_label("Size").bind_chosen(size).set_child(
    Row::new().set_spacing(16.).set_children((
      SimpleRadio::new("s").set_label("Small"),
      SimpleRadio::new("m").set_label("Medium"),
      SimpleRadio::new("l").set_label("Large"),
    )),
  )
}
```

A picker offers the same choice in a popup, and on the web renders as a `<select>`
until the page hydrates:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

fn size(size: Signal<String>) -> impl View {
  SimplePicker::new()
    .set_label("Size")
    .set_options([("s", "Small"), ("m", "Medium"), ("l", "Large")])
    .bind_chosen(size)
}
```
