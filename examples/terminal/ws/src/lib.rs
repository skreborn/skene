//! A WebSocket from a terminal view: a task opens the socket through `skene_transport::ws` on a
//! thread of its own, sends a greeting, and lists each message as it lands.
//!
//! `terminal-ws [url]` opens the URL (a public echo service without one), sends a greeting
//! and lists what comes back. `Ctrl-Q` quits.

#![cfg(not(target_arch = "wasm32"))]

use skene_simple_terminal::prelude::*;
use skene_terminal::prelude::*;

/// The echo service the example talks to when no URL is given.
const DEFAULT_URL: &str = "wss://echo.websocket.org/";

/// The page: the URL, the connection's state, and what came back.
#[must_use]
pub fn app(url: Option<String>) -> impl View {
  let url = url.unwrap_or_else(|| DEFAULT_URL.to_owned());
  let state = Signal::new(String::from("Connecting…"));
  let received = Signal::new(Vec::<String>::new());

  spawn({
    let url = url.clone();

    async move {
      let socket = match skene_transport::ws::connect(url).await {
        Ok(socket) => socket,
        Err(error) => {
          state.set(format!("Failed: {error}"));

          return;
        }
      };

      state.set("Connected; the greeting was sent.".to_owned());

      if let Err(error) = socket.send("hello from skene") {
        state.set(format!("Failed: {error}"));
      }

      while let Some(message) = socket.receive().await {
        let line = match message {
          skene_transport::ws::Message::Text(text) => text,
          skene_transport::ws::Message::Binary(bytes) => format!("{} bytes", bytes.len()),
        };

        received.update(|lines| lines.push(line));
      }

      state.set("Closed.".to_owned());
    }
  });

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_overflow(Overflow::ScrollIfNeeded)
      .set_background(SimplePalette::BACKGROUND)
      .set_child(
        Container::new().set_max_width(72).set_child(
          Column::new()
            .set_spacing(1)
            .set_padding(Inset::symmetric(2, 1))
            .set_children((
              Text::new("WEBSOCKET")
                .set_bold(true)
                .set_color(SimplePalette::MUTED_FOREGROUND),
              Text::new(url.clone()).set_color(SimplePalette::FOREGROUND),
              Text::derive(move || state.get()).set_color(SimplePalette::MUTED_FOREGROUND),
              derived(move || {
                Column::new()
                  .set_children(
                    received
                      .get()
                      .into_iter()
                      .map(|line| Text::new(line).set_color(SimplePalette::FOREGROUND))
                      .collect::<Vec<_>>(),
                  )
                  .build()
              }),
            )),
        ),
      )
  })
}

/// The page over the URL the first argument names.
#[main]
fn from_arguments() -> impl View {
  app(std::env::args().nth(1))
}
