//! The transport in the browser: its `WebSocket`, each event delivered into the
//! receiver.

use std::{
  cell::RefCell,
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context, Poll, Waker},
};

use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{BinaryType, CloseEvent, Event, MessageEvent};

use super::{Error, Message, Receiver};

/// A JavaScript error as a transport error.
fn transport(value: &JsValue) -> Error {
  let message = value.dyn_ref::<js_sys::Error>().map_or_else(
    || format!("{value:?}"),
    |error| String::from(error.message()),
  );

  Error::Transport(message)
}

/// The browser's socket, with the listeners kept alive as long as it is.
pub(crate) struct Link {
  socket: web_sys::WebSocket,
  _listeners: Vec<Closure<dyn FnMut(Event)>>,
}

impl Link {
  pub(crate) fn send(&self, message: Message) -> Result<(), Error> {
    match message {
      Message::Text(text) => self.socket.send_with_str(&text),
      Message::Binary(bytes) => self.socket.send_with_u8_array(&bytes),
    }
    .map_err(|error| transport(&error))
  }

  pub(crate) fn close(&self) {
    let _ = self.socket.close();
  }
}

impl Drop for Link {
  fn drop(&mut self) {
    self.close();
  }
}

/// What the open and error listeners and the future share.
struct Opening {
  outcome: Option<Result<(), Error>>,
  waker: Option<Waker>,
}

/// The connection opening, resolved by the socket's first event.
struct Connect(Rc<RefCell<Opening>>);

impl Future for Connect {
  type Output = Result<(), Error>;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
    let mut opening = self.0.borrow_mut();

    if let Some(outcome) = opening.outcome.take() {
      return Poll::Ready(outcome);
    }

    opening.waker = Some(context.waker().clone());

    Poll::Pending
  }
}

/// Settles `opening` with `outcome`, once.
fn settle(opening: &Rc<RefCell<Opening>>, outcome: Result<(), Error>) {
  let waker = {
    let mut opening = opening.borrow_mut();

    if opening.outcome.is_some() {
      return;
    }

    opening.outcome = Some(outcome);
    opening.waker.take()
  };

  if let Some(waker) = waker {
    waker.wake();
  }
}

/// Opens `url` and resolves once the socket is open, its messages delivered into
/// `receiver` from then on.
pub(crate) async fn connect(url: String, receiver: Receiver) -> Result<Link, Error> {
  let socket = web_sys::WebSocket::new(&url).map_err(|error| Error::Url(format!("{error:?}")))?;

  socket.set_binary_type(BinaryType::Arraybuffer);

  let opening = Rc::new(RefCell::new(Opening {
    outcome: None,
    waker: None,
  }));
  let mut listeners: Vec<Closure<dyn FnMut(Event)>> = Vec::new();

  {
    let opening = Rc::clone(&opening);
    let on_open = Closure::<dyn FnMut(Event)>::new(move |_: Event| settle(&opening, Ok(())));

    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    listeners.push(on_open);
  }

  {
    let opening = Rc::clone(&opening);
    let on_error = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
      settle(
        &opening,
        Err(Error::Transport("the socket reported an error".to_owned())),
      );
    });

    socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    listeners.push(on_error);
  }

  {
    let receiver = receiver.clone();
    let on_message = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
      let Some(event) = event.dyn_ref::<MessageEvent>()
      else {
        return;
      };
      let data = event.data();

      if let Some(text) = data.as_string() {
        receiver.deliver(Message::Text(text));
      }
      else if let Some(buffer) = data.dyn_ref::<ArrayBuffer>() {
        receiver.deliver(Message::Binary(Uint8Array::new(buffer).to_vec()));
      }
    });

    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    listeners.push(on_message);
  }

  {
    let opening = Rc::clone(&opening);
    let on_close = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
      let reason = event
        .dyn_ref::<CloseEvent>()
        .map_or_else(String::new, |close| {
          format!("{} {}", close.code(), close.reason())
        });

      settle(&opening, Err(Error::Transport(format!("closed: {reason}"))));
      receiver.close();
    });

    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    listeners.push(on_close);
  }

  Connect(opening).await?;

  Ok(Link {
    socket,
    _listeners: listeners,
  })
}
