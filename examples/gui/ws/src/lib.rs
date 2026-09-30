//! A WebSocket from a view: a task opens a socket to a public echo service through
//! `skene_transport::ws`, a button sends what the field holds, and every message received is
//! listed.
//!
//! The socket is opened by a client-only resource, so the server render carries the
//! connecting caption and the bundle connects on arrival. Messages land through the
//! receiving future in a task of its own and are written into a signal the list
//! follows.

use std::rc::Rc;

use skene_gui::{HasChild, Resource, prelude::*, spawn};
use skene_simple::gui::prelude::*;

/// The echo service the example talks to.
const ECHO: &str = "wss://echo.websocket.org/";

#[main]
fn app() -> impl View {
  Theme::new(SimplePalette::default()).set_child_builder(|| {
    Container::new()
      .set_role(Role::Main)
      .set_color(SimplePalette::BACKGROUND)
      .set_min_height(Dimension::Fraction(1.))
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_padding(Inset::symmetric(24., 48.))
          .add_child(
            Container::new().set_max_width(460.).set_child(
              Column::new()
                .set_spacing(24.)
                .set_children((heading(), echo_card())),
            ),
          ),
      )
  })
}

fn heading() -> impl View {
  Column::new().set_spacing(6.).set_children((
    Text::new("WEBSOCKET")
      .set_font_size(13.)
      .set_font_weight(500.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("A socket is opened by a task; its messages land in a signal.").set_font_size(15.),
  ))
}

/// The socket to the echo service, a field sending into it, and what came back.
fn echo_card() -> impl View {
  let socket = Signal::transient(None::<Rc<skene_transport::ws::WebSocket>>);
  let received = Signal::transient(Vec::<String>::new());
  let draft = Signal::new(String::from("hello from skene"));
  let failure = Signal::transient(None::<String>);
  let send = move || {
    let Some(socket) = socket.get()
    else {
      return;
    };

    if let Err(error) = socket.send(draft.get()) {
      failure.set(Some(error.to_string()));
    }
  };
  let opening: Resource<Result<(), String>> = Resource::client(move || async move {
    let opened = skene_transport::ws::connect(ECHO)
      .await
      .map(Rc::new)
      .map_err(|error| error.to_string())?;

    socket.set(Some(Rc::clone(&opened)));

    // Receiving runs for the socket's whole life, in a task of its own.
    spawn(async move {
      while let Some(message) = opened.receive().await {
        let line = match message {
          skene_transport::ws::Message::Text(text) => text,
          skene_transport::ws::Message::Binary(bytes) => format!("{} bytes", bytes.len()),
        };

        received.update(|lines| lines.push(line));
      }

      socket.set(None);
    });

    Ok(())
  });
  let state = move || match (failure.get(), opening.get()) {
    (Some(error), _) | (None, Some(Err(error))) => format!("Failed: {error}"),
    (None, None) => "Connecting…".to_owned(),
    (None, Some(Ok(()))) if socket.with(Option::is_none) => "Closed.".to_owned(),
    (None, Some(Ok(()))) => "Connected; the service echoes what is sent.".to_owned(),
  };

  SimpleCard::new().set_spacing(12.).set_children((
    Text::new("Echo").set_font_weight(600.),
    Text::derive(state)
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    SimpleTextField::new()
      .set_placeholder("A message")
      .bind_value(draft),
    SimpleButton::new("Send").handle_click(send),
    derived(move || {
      Column::new()
        .set_spacing(4.)
        .set_children(
          received
            .get()
            .into_iter()
            .map(|line| Text::new(line).set_font_size(14.))
            .collect::<Vec<_>>(),
        )
        .build()
    }),
  ))
}
