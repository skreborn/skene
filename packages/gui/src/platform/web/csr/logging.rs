//! A `tracing` subscriber that routes events to the browser console.

use std::fmt::{self, Write};

use tracing::{
  Event,
  Level,
  Metadata,
  Subscriber,
  field::{Field, Visit},
  span,
};
use wasm_bindgen::JsValue;

/// Routes each `tracing` event to the browser console, by level: `error!` to
/// `console.error`, `warn!` to `console.warn`, everything else to `console.log`. Spans are
/// ignored.
pub(super) struct ConsoleSubscriber;

impl Subscriber for ConsoleSubscriber {
  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn new_span(&self, _span: &span::Attributes<'_>) -> span::Id {
    span::Id::from_u64(1)
  }

  fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}

  fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

  fn event(&self, event: &Event<'_>) {
    let metadata = event.metadata();
    let mut line = format!("{}: ", metadata.target());

    event.record(&mut FieldVisitor(&mut line));

    let value = JsValue::from_str(&line);

    match *metadata.level() {
      Level::ERROR => web_sys::console::error_1(&value),
      Level::WARN => web_sys::console::warn_1(&value),
      _ => web_sys::console::log_1(&value),
    }
  }

  fn enter(&self, _span: &span::Id) {}

  fn exit(&self, _span: &span::Id) {}
}

/// Appends an event's fields to a line: the `message` field unlabelled, others as
/// `name=value`.
struct FieldVisitor<'a>(&'a mut String);

impl Visit for FieldVisitor<'_> {
  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    if field.name() == "message" {
      let _ = write!(self.0, "{value:?}");
    }
    else {
      let _ = write!(self.0, " {}={value:?}", field.name());
    }
  }
}
