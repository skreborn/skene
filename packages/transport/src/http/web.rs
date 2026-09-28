//! The transport in the browser: `fetch`, the body read whole.

use js_sys::{Array, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, RequestInit};

use super::{Error, Request, Response};

/// A JavaScript error as a transport error.
fn transport(value: &JsValue) -> Error {
  let message = value.dyn_ref::<js_sys::Error>().map_or_else(
    || format!("{value:?}"),
    |error| String::from(error.message()),
  );

  Error::Transport(message)
}

/// Sends `request` through `fetch` and resolves with the whole response.
pub(crate) async fn send(request: Request) -> Result<Response, Error> {
  let window = web_sys::window().ok_or_else(|| Error::Transport("no window".to_owned()))?;
  let init = RequestInit::new();
  let headers = Headers::new().map_err(|error| transport(&error))?;

  for (name, value) in request.headers() {
    headers
      .append(name, value)
      .map_err(|error| transport(&error))?;
  }

  init.set_method(request.method().as_str());
  init.set_headers(&headers);

  if !request.body().is_empty() {
    init.set_body(&Uint8Array::from(request.body()));
  }

  let outgoing = web_sys::Request::new_with_str_and_init(request.url(), &init)
    .map_err(|error| transport(&error))?;
  let incoming: web_sys::Response = JsFuture::from(window.fetch_with_request(&outgoing))
    .await
    .map_err(|error| transport(&error))?
    .dyn_into()
    .map_err(|error| transport(&error))?;
  let status = incoming.status();
  let mut received = Vec::new();

  if let Some(entries) = js_sys::try_iter(&incoming.headers()).map_err(|error| transport(&error))? {
    for entry in entries {
      let entry: Array = entry.map_err(|error| transport(&error))?.into();

      received.push((
        entry.get(0).as_string().unwrap_or_default(),
        entry.get(1).as_string().unwrap_or_default(),
      ));
    }
  }

  let buffer = JsFuture::from(incoming.array_buffer().map_err(|error| transport(&error))?)
    .await
    .map_err(|error| transport(&error))?;
  let body = Uint8Array::new(&buffer).to_vec();

  Ok(Response::new(status, received, body))
}
