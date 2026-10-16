//! The clipboard and the primary selection from a focused box: `Ctrl+C` offers a text as the
//! clipboard and `Ctrl+Shift+C` as the primary selection, `Ctrl+V` and `Ctrl+Shift+V` read
//! them back into the window, each logged on standard output.

use skene_gui::{
  KeyEvent,
  platform::linux::clipboard::{self, Selection},
  prelude::*,
};

#[skene_gui::main]
fn app() -> impl View {
  let pasted = Signal::new(String::from("(nothing)"));
  let offered = Signal::new(0u32);

  Container::new()
    .set_padding(16)
    .set_background(Color::rgb(0xffffff))
    .handle_click(|| {})
    .handle_key(move |event: &KeyEvent| {
      if !event.down || !event.modifiers.control {
        return Bubbling::Continue;
      }

      let selection = if event.modifiers.shift {
        Selection::Primary
      } else {
        Selection::Clipboard
      };

      match event.code.as_str() {
        "KeyC" => {
          offered.update(|count| *count += 1);

          let text = format!("Skene offered {selection:?} {}", offered.get_untracked());

          println!("offered {text:?}");
          clipboard::write(selection, text);
          Bubbling::Stop
        }
        "KeyV" => {
          clipboard::read(selection, move |text| {
            println!("pasted {selection:?} {text:?}");
            pasted.set(text.unwrap_or_else(|| "(empty)".to_owned()));
          });
          Bubbling::Stop
        }
        _ => Bubbling::Continue,
      }
    })
    .set_child(
      Column::new()
        .set_spacing(8)
        .add_child(Text::derive(move || format!("Offered: {}", offered.get())))
        .add_child(Text::derive(move || format!("Pasted: {}", pasted.get()))),
    )
}
