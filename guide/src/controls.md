# Controls

A control is a view a user operates: a button, a checkbox, a slider, a text field, a
picker. The facade draws its own controls in the platform's colors: `Button`,
`Checkbox`, `Toggle` (a switch), `RadioGroup` with its `Radio`s, `Slider`, `TextField`,
`TextEditor`, `Picker` and `Listbox` with their `ListboxOption`s. Each comes styled,
every default a value its setters replace, so a library's preset is a built-in it
styles further:

```rust
use skene_web::prelude::*;

fn settings(name: Signal<String>, agreed: Signal<bool>) -> impl View {
  Column::new().set_spacing(12).set_children((
    TextField::new()
      .set_label("Name")
      .set_placeholder("Your name")
      .bind_value(name),
    Checkbox::new().set_label("I agree").bind_checked(agreed),
    Button::new("Save").handle_activate(move || println!("saved {}", name.get())),
  ))
}
```

## The built-in controls

A button's label is its content and its name, and `set_prominent` marks the screen's
primary action, which each platform shows in its own idiom: the accent's fill in pixels,
accent cells in the terminal. A derived prominence re-inks the button in place:

```rust
use skene_web::prelude::*;

fn form(dirty: Signal<bool>, notify: Signal<bool>, sync: Signal<bool>) -> impl View {
  Column::new().set_spacing(12).set_children((
    Checkbox::new().set_label("Email me").bind_checked(notify),
    Toggle::new().set_label("Sync").bind_checked(sync),
    Row::new().set_spacing(8).set_children((
      Button::new("Cancel"),
      Button::new("Save")
        .derive_prominent(move || dirty.get())
        .set_padding(Inset::symmetric(20, 4))
        .handle_activate(move || dirty.set(false)),
    )),
  ))
}
```

A radio group is typed by the app's own value, each radio tagged with one, and lays
the radios it is given out in a column beneath its caption; a slider fills its track to
its thumb as its value moves:

```rust
use skene_web::{behavior::SliderRange, prelude::*};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Quality {
  Draft,
  Final,
}

fn export(quality: Signal<Quality>, scale: Signal<f32>) -> impl View {
  Column::new().set_spacing(12).set_children((
    RadioGroup::new()
      .set_label("Quality")
      .bind_chosen(quality)
      .set_children((
        Radio::new(Quality::Draft).set_label("Draft"),
        Radio::new(Quality::Final).set_label("Final"),
      )),
    Slider::new()
      .set_label("Scale")
      .set_range(SliderRange::new(50., 200., 10.))
      .bind_value(scale),
  ))
}
```

A text field and a text editor are filled with the platform's field color inside an
outline that takes the accent while focus shows, their value selected in the platform's
selection color; on the web each is the page's own `<input>` or `<textarea>`, styled by
CSS. A picker shows the chosen option's name and an indicator, and opens its options in
a list; a listbox shows them all. Both take `ListboxOption`s, each tagged with a value
of the app's own type and named by its caption, a check showing on the chosen one:

```rust
use skene_web::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Unit {
  Pixels,
  Points,
}

fn layout(notes: Signal<String>, unit: Signal<Unit>, tag: Signal<Option<u32>>) -> impl View {
  Column::new().set_spacing(12).set_children((
    TextEditor::new()
      .set_label("Notes")
      .set_lines(4)
      .set_selection_background(Color::srgb(1., 0.9, 0.4))
      .bind_value(notes),
    Picker::new()
      .set_label("Unit")
      .bind_chosen(unit)
      .set_options((
        ListboxOption::new(Unit::Pixels).set_label("Pixels"),
        ListboxOption::new(Unit::Points).set_label("Points"),
      )),
    Listbox::new()
      .set_label("Tag")
      .bind_chosen(tag)
      .set_children((
        ListboxOption::new(1_u32).set_label("Home"),
        ListboxOption::new(2_u32).set_label("Work"),
      )),
  ))
}
```

A checkbox's, a radio's and an option's mark, a switch's track and a slider's track are
parts (`Part::Mark`, `Part::Track`), and `set_part` replaces the default one with a view
of the app's own; a radio group's or a listbox's `set_child` replaces its column with a
layout of the app's own, a picker's `set_face` draws its face, and a picker's or a
listbox's `set_list` gives its options a box of the app's own.

## The bound signal is the truth

A control reports what its user does, and its signal decides what it shows. `bind_value`
wires both to one signal; `derive_value` and `handle_change` take them apart, so a
handler can refuse a change and the control returns to the signal's value:

```rust
use skene_web::prelude::*;

fn digits(code: Signal<String>) -> impl View {
  TextField::new()
    .set_label("Code")
    .derive_value(move || code.get())
    .handle_change(move |typed: &str| {
      if typed.chars().all(|character| character.is_ascii_digit()) {
        code.set(typed.to_owned());
      }
    })
}
```

The selection is controlled beside the value: `bind_selection` wires a field's
selection, in grapheme clusters of its value, to a `Signal<TextSelection>`, and
`derive_selection` and `handle_selection_change` take it apart the same way.

## Secure fields and keyboards

`set_secure` makes a field hold a secret: its text is masked, a mark for each character,
and kept from the clipboard, copying and cutting disabled in it as in a browser's
password field, while pasting stays. `set_keyboard` tells the platform what the field
takes, so its input method offers the keyboard, the autofill and the corrections that
suit it: an email address, a number, a telephone number, a URL or a query, what the
platform may fill it from (`Autofill`), whether what is typed is corrected and
capitalized, and what the `Enter` key says. Each platform carries what its input
protocol has a word for: the web's `<input>` becomes a password input and takes the
keyboard as `inputmode`, `autocomplete` and `enterkeyhint`, and the terminal masks a
secure field in its cells and has no keyboard to offer.

```rust
use skene_web::prelude::*;

fn sign_in(email: Signal<String>, password: Signal<String>) -> impl View {
  Column::new().set_spacing(12).set_children((
    TextField::new()
      .set_label("Email")
      .set_keyboard(Keyboard::EMAIL.with_autofill(Autofill::Email))
      .bind_value(email),
    TextField::new()
      .set_label("Password")
      .set_secure(true)
      .set_keyboard(
        Keyboard::TEXT
          .with_autofill(Autofill::Password)
          .with_submit(SubmitLabel::Go),
      )
      .bind_value(password),
  ))
}
```

## A control owns its events

A control consumes the events it acts on: a press on a button is a press on that button,
never also on the row it sits in. A handler takes a plain callback, and the control
decides what the platform and its ancestors are told. A `Container` is a box rather than
a control, and its handlers choose whether an event travels on, so a group that must
observe its controls' clicks in one place wraps them in one. A `Container` that handles a
click is an activatable row: it takes focus and `Enter` and `Space` activate it, while
the controls inside it own their presses and keys, so a button in a row never also
activates the row.

## Focus

`Tab` moves focus through the controls in the order the views are built in, the order a
reader reads them; `set_focus_order(FocusOrder::Skip)` keeps a control out of that
order, still focusable by a click. A window opens with nothing focused. A control set to
autofocus takes focus as its window opens, the first of them in that order where several
ask, one inside a hidden box passed over; a radio group or a listbox set to autofocus
focuses its chosen member, or its first. A server-rendered page carries it as the `autofocus` attribute, so the page
opens focused on it before its script arrives:

```rust
use skene_web::prelude::*;

fn search() -> impl View {
  Column::new().set_spacing(12).set_children((
    TextField::new().set_label("Search").set_autofocus(true),
    Button::new("Go"),
  ))
}
```

## Shortcuts

`set_shortcut` gives a control the key that activates it from anywhere in the window, as
a click does, and the platform reports it to assistive technology (`aria-keyshortcuts`
on the web). It acts while the control is enabled and shown and no open modal it lies
outside holds the window; where several controls take one key, the first of them in the
window, in the order of the views' children, acts. A key without `Ctrl`, `Alt` or `Meta`
stays a text entry's own while one holds focus, so a shortcut on `N` types an `n` there,
and a `Keymap` binding the same key inside its view acts first there. A radio group's or
a listbox's shortcut moves focus to its chosen member, or its first, rather than
activating it. An activatable row takes a shortcut the same way as a control:

```rust
use skene_web::prelude::*;

fn editor(save: impl Fn() + 'static) -> impl View {
  Column::new().set_spacing(12).set_children((
    TextField::new().set_label("Note"),
    Button::new("Save")
      .set_shortcut(Shortcut::new(Modifiers::CTRL, "s"))
      .handle_activate(save),
  ))
}
```

## Labels and accessibility

A control's label sits where its kind places it: beside a checkbox's box, before a text
field. A label is a convenience: `set_labelled_by` names a control by a caption placed
anywhere, and `set_accessible_name` by text alone. What a control is (checked, expanded,
its value) is reported by its behavior, and each platform maps it to its own layer: on
the web a form element takes the native attribute, so a form enforces it without
script.

A disabled control takes no focus.

## Groups

A radio group holds its radios as children and binds the chosen one. What it chooses is
a value of the app's own type, any type that is `Clone`, `Eq` and `Hash`, each radio
tagged with one. A group that starts with nothing chosen binds an `Option<Size>`, its
radios tagged `Some(Size::Small)` and so on. A listbox binds an `Option` the same way.

A picker offers the same choice in a popup, and on the web renders as a `<select>`
until the page hydrates; typing over it picks the option whose name the characters
begin, the name its label supplies. The page the server renders names each option by a
key both the server and the browser give it alike, so a choice the reader makes before
the script arrives reaches the `Signal<Size>` once it does.

## Beneath the built-ins

A built-in is a `Control` over a behavior, which the facade's `behavior` module holds
(`behavior::TextEntry` beneath a text field, `behavior::Activate` beneath a button).
The `Basic*` controls are those controls undecorated, for a look built from nothing:

```rust
use skene_web::{behavior::TextEntry, prelude::*};

fn bare() -> impl View {
  BasicTextField::new(TextEntry::new().set_placeholder("Search"))
    .set_label("Search")
    .set_padding(Inset::all(2))
}
```

## The Simple library

A component library dresses the controls in a design language of its own. Simple's
controls come drawn and ready, in the platform's crate of the Simple library, each
inked from its palette:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Size {
  Small,
  Medium,
  Large,
}

fn order(name: Signal<String>, size: Signal<Size>) -> impl View {
  Column::new().set_spacing(12).set_children((
    SimpleTextField::new()
      .set_label("Name")
      .set_placeholder("Your name")
      .bind_value(name),
    SimpleRadioGroup::new().set_label("Size").bind_chosen(size).set_child(
      Row::new().set_spacing(16).set_children((
        SimpleRadio::new(Size::Small).set_label("Small"),
        SimpleRadio::new(Size::Medium).set_label("Medium"),
        SimpleRadio::new(Size::Large).set_label("Large"),
      )),
    ),
    SimplePicker::new()
      .set_label("Size")
      .bind_chosen(size)
      .set_options((
        SimpleListboxOption::new(Size::Small, "Small"),
        SimpleListboxOption::new(Size::Medium, "Medium"),
        SimpleListboxOption::new(Size::Large, "Large"),
      )),
    SimpleButton::new("Order").handle_activate(move || println!("ordered {}", name.get())),
  ))
}
```

Every Simple component is a preset: each constructor returns the facade's own built-in,
styled, so every member of the built-in applies. `SimpleTextField::new()` is a
`TextField`, `SimpleSlider::new()` a `Slider`, `SimpleContextMenu::new()` a
`ContextMenu`, `SimpleRadioGroup::new()` a `RadioGroup`, `SimplePicker::new()` a
`Picker`, and `SimpleListboxOption::new(value, caption)` a `ListboxOption` whose caption
names it for the face and for type-ahead. A slider's `set_label` is the built-in's
visible caption, so a Simple slider nothing visible names takes its name from
`set_accessible_name`. A
button takes a constructor per variant, and `derive_variant` follows a signal, re-inking
the button in place:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

fn actions(dirty: Signal<bool>) -> impl View {
  Row::new().set_spacing(8).set_children((
    SimpleButton::outline("Cancel"),
    SimpleButton::derive_variant("Save", move || {
      if dirty.get() {
        SimpleVariant::Primary
      }
      else {
        SimpleVariant::Secondary
      }
    }),
    SimpleButton::destructive("Delete").set_disabled(true),
  ))
}
```

`SimpleButton::styled(button, variant)` dresses a button built otherwise, a derived
label's (`Button::derive`) or one drawn by an icon, and `SimpleCard::new()` is a
`Container` around the one child it is given, a `Column` spaced `SimpleCard::spacing()`
apart where it groups several. Every preset has its `styled` beside it, and the pieces it
composes, a checkbox's or a radio's `mark`, a picker's `face` and `list`, an option's
`check` and `label`, a slider's `track` and `thumb`, are public, so a library composes
them differently. A mark follows
the control's state rather than a value it reads, so the checked state may be bound
after the preset is made.
