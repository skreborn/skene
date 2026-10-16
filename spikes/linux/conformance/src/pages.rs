//! The suite's pages: views built with skene-gui's shared views alone, each publishing probes
//! of the signals it binds (the only state a scenario reads that a reader does not).

use acme_rating::star_rating;
use skene_gui::{
  PointerEvent, PointerPhase,
  painted::shapes::Rect,
  prelude::*,
  testing::{publish, register as register_page},
};
use skene_simple_gui::{
  SimpleButton, SimpleCheckbox, SimpleDate, SimpleDatePicker, SimplePalette, SimpleToggle,
};

/// Every page's name.
pub const NAMES: &[&str] = &[
  "toggles", "text", "sliders", "picker", "box", "focus", "order", "rows", "shortcut", "scheme",
  "width", "library", "painted", "buttons", "checkbox", "rings", "disabled", "simple", "accent",
  "fruit", "caption", "editor", "rating",
];

/// Registers every page under its name.
pub fn register() {
  register_page("toggles", toggles);
  register_page("text", text);
  register_page("sliders", sliders);
  register_page("picker", picker);
  register_page("box", pressed_box);
  register_page("focus", focus);
  register_page("order", order);
  register_page("rows", rows);
  register_page("shortcut", shortcut);
  register_page("scheme", scheme);
  register_page("width", width);
  register_page("library", library);
  register_page("painted", painted);
  register_page("buttons", buttons);
  register_page("checkbox", checkbox);
  register_page("rings", rings);
  register_page("disabled", disabled);
  register_page("simple", simple);
  register_page("accent", accent);
  register_page("fruit", fruit);
  register_page("caption", caption);
  register_page("editor", editor);
  register_page("rating", rating);
}

/// Publishes `signal`'s value as the probe `name`, as `Display` writes it.
fn probe<T: Clone + std::fmt::Display + 'static>(name: &'static str, signal: Signal<T>) {
  publish(name, move || signal.get_untracked().to_string());
}

/// A switch whose handler takes each flip, and one whose handler takes none.
fn toggles() -> impl View {
  let wifi = Signal::new(false);
  let locked = Signal::new(false);
  let asked = Signal::new(0_u32);

  probe("wifi", wifi);
  probe("locked", locked);
  probe("locked asked", asked);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Toggle::new("Wi-Fi").bind_on(wifi))
    .add_child(
      Toggle::new("Locked")
        .derive_on(move || locked.get())
        .handle_change(move |_| asked.update(|asked| *asked += 1)),
    )
}

/// A field that takes what is typed, one that makes each edit upper case without digits, and
/// one that refuses an edit holding an `x`.
fn text() -> impl View {
  let name = Signal::new(String::new());
  let code = Signal::new(String::new());
  let letters = Signal::new(String::new());

  probe("name", name);
  probe("code", code);
  probe("letters", letters);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Name").bind_text(name))
    .add_child(
      TextField::new("Code")
        .derive_text(move || code.get())
        .handle_change(move |proposed| {
          code.set(
            proposed
              .chars()
              .filter(|character| !character.is_ascii_digit())
              .collect::<String>()
              .to_uppercase(),
          );
        }),
    )
    .add_child(
      TextField::new("Letters")
        .derive_text(move || letters.get())
        .handle_change(move |proposed| {
          if !proposed.contains('x') {
            letters.set(proposed);
          }
        }),
    )
}

/// A slider that takes each step, and one whose handler holds it at 40 or below.
fn sliders() -> impl View {
  let volume = Signal::new(30.0_f32);
  let limited = Signal::new(10.0_f32);

  probe("volume", volume);
  probe("limited", limited);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Slider::new("Volume")
        .set_range(0., 100.)
        .set_step(1.)
        .bind_value(volume),
    )
    .add_child(
      Slider::new("Limited")
        .set_range(0., 100.)
        .set_step(1.)
        .derive_value(move || limited.get())
        .handle_change(move |value| limited.set(value.min(40.))),
    )
}

/// A choice of three.
fn picker() -> impl View {
  let size = Signal::new(Some(0_usize));

  publish("size", move || format!("{:?}", size.get_untracked()));

  Column::new().set_padding(16).add_child(
    Picker::new("Size")
      .set_options(["Small", "Medium", "Large"])
      .bind_chosen(size),
  )
}

/// A box that hears presses, holding a text and a button.
fn pressed_box() -> impl View {
  let presses = Signal::new(0_u32);
  let inside = Signal::new(0_u32);

  probe("box presses", presses);
  probe("inside", inside);

  Column::new().set_padding(16).add_child(
    Container::new()
      .set_padding(16)
      .set_label("Card")
      .handle_pointer(move |event: &PointerEvent| {
        if event.phase == PointerPhase::Up {
          presses.update(|presses| *presses += 1);
        }
      })
      .set_child(
        Row::new()
          .set_spacing(12)
          .set_item_alignment(AxisAlignment::Center)
          .add_child(
            Flexible::new()
              .set_flexibility(1)
              .set_child(Text::new("Card body")),
          )
          .add_child(
            Button::new("Inside").handle_activate(move || inside.update(|inside| *inside += 1)),
          ),
      ),
  )
}

/// Four controls in reading order.
fn focus() -> impl View {
  let first = Signal::new(String::new());
  let second = Signal::new(false);
  let fourth = Signal::new(5.0_f32);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("First").bind_text(first))
    .add_child(Toggle::new("Second").bind_on(second))
    .add_child(Button::new("Third"))
    .add_child(Slider::new("Fourth").bind_value(fourth))
}

/// The swatch's color: white, black where the scheme is dark.
const SWATCH: TokenRef<Color> = TokenRef::new("conformance-swatch");

/// The scheme page's palette.
struct Swatches(Token<Color>);

impl TokenSet for Swatches {
  fn define(&self, definitions: &mut Definitions) {
    definitions.add(&self.0);
  }
}

/// A swatch in a token's color, and a field holding what was typed into it.
fn scheme() -> impl View {
  let kept = Signal::new(String::new());

  probe("kept", kept);

  Theme::new(Swatches(
    Token::new(SWATCH, Color::WHITE).when(Media::Dark, Color::BLACK),
  ))
  .set_child(
    Column::new()
      .set_padding(16)
      .set_spacing(12)
      .add_child(
        Container::new()
          .set_label("Swatch")
          .set_width(60)
          .set_height(60)
          .set_background(SWATCH),
      )
      .add_child(TextField::new("Kept").bind_text(kept)),
  )
}

/// A frame whose padding is a width condition, and a text that follows the width.
fn width() -> impl View {
  let wide = media::watch(Media::MinWidth(600));

  Container::new()
    .set_label("Frame")
    .set_padding(8.when(Media::MinWidth(600), 32))
    .set_child(
      Column::new()
        .set_spacing(4)
        .add_child(Container::new().set_label("Inner").set_height(20))
        .add_child(Text::derive(
          move || if wide.get() { "wide" } else { "narrow" },
        )),
    )
}

/// A field, then three buttons in a grid that draws the scene's second before its first.
fn order() -> impl View {
  let start = Signal::new(String::new());

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Start").bind_text(start))
    .add_child(
      Grid::new()
        .set_spacing(8)
        .set_columns([GridTrack::Auto, GridTrack::Auto, GridTrack::Auto])
        .add_child(
          GridCell::new()
            .set_cell(GridPlacement::at(1, 2))
            .set_child(Button::new("One")),
        )
        .add_child(
          GridCell::new()
            .set_cell(GridPlacement::at(1, 1))
            .set_child(Button::new("Two")),
        )
        .add_child(
          GridCell::new()
            .set_cell(GridPlacement::at(1, 3))
            .set_child(Button::new("Three")),
        ),
    )
}

/// An activatable row holding a text and a switch, after a field.
fn rows() -> impl View {
  let before = Signal::new(String::new());
  let opened = Signal::new(0_u32);
  let muted = Signal::new(false);

  probe("opened", opened);
  probe("muted", muted);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Before").bind_text(before))
    .add_child(
      Container::new()
        .set_padding(12)
        .set_label("Inbox")
        .handle_click(move || opened.update(|opened| *opened += 1))
        .set_child(
          Row::new()
            .set_spacing(12)
            .set_item_alignment(AxisAlignment::Center)
            .add_child(
              Flexible::new()
                .set_flexibility(1)
                .set_child(Text::new("Inbox")),
            )
            .add_child(
              Toggle::new("Muted")
                .set_accessible_name("Mute Inbox")
                .bind_on(muted),
            ),
        ),
    )
}

/// A field, and a button its shortcut (`Ctrl+S`) activates wherever focus is.
fn shortcut() -> impl View {
  let title = Signal::new(String::new());
  let saved = Signal::new(0_u32);

  probe("saved", saved);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Title").bind_text(title))
    .add_child(
      Button::new("Save")
        .set_shortcut(Shortcut::primary('s'))
        .handle_activate(move || saved.update(|saved| *saved += 1)),
    )
}

/// A library's date picker (Simple's), controlled, that refuses weekends.
fn library() -> impl View {
  let due = Signal::new("2026-10-14".to_owned());
  let picks = Signal::new(0_u32);

  probe("due", due);
  probe("picks", picks);

  Column::new().set_padding(16).add_child(
    SimpleDatePicker::new("Due")
      .derive_date(move || due.get())
      .handle_change(move |date| {
        picks.update(|picks| *picks += 1);

        if SimpleDate::parse(&date).is_some_and(|date| !date.is_weekend()) {
          due.set(date);
        }
      }),
  )
}

/// A described picture, and one with no description.
fn painted() -> impl View {
  let paint = |canvas: &mut Canvas<'_>| {
    let (width, height) = (f64::from(canvas.width()), f64::from(canvas.height()));

    canvas.fill(&Rect::new(0., 0., width / 2., height), Color::rgb(0xff0000));
    canvas.fill(
      &Rect::new(width / 2., 0., width, height),
      Color::rgb(0x0000ff),
    );
  };

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Painted::new(paint)
        .set_width(200)
        .set_height(50)
        .set_description("Sales by half"),
    )
    .add_child(Painted::new(paint).set_width(200).set_height(50))
}

/// A field, then a button counting its activations.
fn buttons() -> impl View {
  let start = Signal::new(String::new());
  let acts = Signal::new(0_u32);

  probe("acts", acts);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Start").bind_text(start))
    .add_child(Button::new("Act").handle_activate(move || acts.update(|acts| *acts += 1)))
}

/// A checkbox, alone in a row so its caption is as wide as its text.
fn checkbox() -> impl View {
  let remember = Signal::new(false);

  probe("remember", remember);

  Column::new()
    .set_padding(16)
    .add_child(Row::new().add_child(Checkbox::new("Remember me").bind_checked(remember)))
}

/// A field, a button showing the default focus ring, and one whose ring is none.
fn rings() -> impl View {
  let start = Signal::new(String::new());

  Column::new()
    .set_padding(16)
    .set_spacing(16)
    .add_child(TextField::new("Start").bind_text(start))
    .add_child(Row::new().add_child(Button::new("Ringed")))
    .add_child(Row::new().add_child(Button::new("Plain").set_ring(Ring::NONE)))
}

/// A field, a disabled checkbox, and an enabled one.
fn disabled() -> impl View {
  let start = Signal::new(String::new());
  let off = Signal::new(false);
  let on = Signal::new(false);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Start").bind_text(start))
    .add_child(Checkbox::new("Off").set_disabled(true).bind_checked(off))
    .add_child(Checkbox::new("On").bind_checked(on))
}

/// Simple's button, checkbox and toggle, and a plain switch, beneath Simple's palette.
fn simple() -> impl View {
  let agree = Signal::new(true);
  let sync = Signal::new(true);
  let plain = Signal::new(true);

  Theme::new(SimplePalette::default()).set_child(
    Column::new()
      .set_padding(16)
      .set_spacing(12)
      .add_child(Row::new().add_child(SimpleButton::new("Save")))
      .add_child(Row::new().add_child(SimpleCheckbox::new("Agree").bind_checked(agree)))
      .add_child(SimpleToggle::new("Sync").bind_on(sync))
      .add_child(Toggle::new("Plain").bind_on(plain)),
  )
}

/// A theme defining the platform's accent, orange or green.
struct Accent(Color);

impl TokenSet for Accent {
  fn define(&self, definitions: &mut Definitions) {
    definitions.add(&Token::new(SystemColors::ACCENT, self.0));
  }
}

/// A switch, a checkbox, a full slider and a field beneath a theme redefining the accent, and
/// a button flipping it from orange to green.
fn accent() -> impl View {
  let green = Signal::new(false);
  let sync = Signal::new(true);
  let mail = Signal::new(true);
  let level = Signal::new(1.0_f32);
  let note = Signal::new(String::new());

  probe("note", note);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(
      Button::new("Flip accent").handle_activate(move || green.update(|green| *green = !*green)),
    )
    .add_child(
      Theme::derive(move || {
        Accent(if green.get() {
          Color::rgb(0x34c759)
        } else {
          Color::rgb(0xff9500)
        })
      })
      .set_child(
        Column::new()
          .set_spacing(12)
          .add_child(Toggle::new("Sync").bind_on(sync))
          .add_child(Row::new().add_child(Checkbox::new("Mail").bind_checked(mail)))
          .add_child(Slider::new("Level").bind_value(level))
          .add_child(TextField::new("Note").bind_text(note)),
      ),
    )
}

/// A choice of five fruit, in a row so the picker is as wide as it likes.
fn fruit() -> impl View {
  let fruit = Signal::new(Some(1_usize));

  publish("fruit", move || format!("{:?}", fruit.get_untracked()));

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Text::new("Pick one"))
    .add_child(
      Row::new().add_child(
        Picker::new("Fruit")
          .set_options(["Apple", "Banana", "Cherry", "Date", "Elderberry"])
          .bind_chosen(fruit),
      ),
    )
}

/// A field whose caption sits beside it.
fn caption() -> impl View {
  let email = Signal::new(String::new());

  probe("email", email);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(TextField::new("Email").bind_text(email))
}

/// An editor 120 pixels tall at most.
fn editor() -> impl View {
  let notes = Signal::new(String::new());

  Column::new().set_padding(16).set_spacing(12).add_child(
    TextEditor::new("Notes")
      .set_max_height(120)
      .bind_text(notes),
  )
}

/// A library's drawn control (acme-rating's), between two buttons, and a disabled copy.
fn rating() -> impl View {
  let stars = Signal::new(0_u8);
  let locked = Signal::new(4_u8);

  probe("stars", stars);

  Column::new()
    .set_padding(16)
    .set_spacing(12)
    .add_child(Button::new("Before"))
    .add_child(Row::new().add_child(star_rating("Rating", stars)))
    .add_child(Row::new().add_child(star_rating("Locked", locked).set_disabled(true)))
    .add_child(Button::new("After"))
}
