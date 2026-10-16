//! Fcitx5 over D-Bus on the session bus (`org.fcitx.Fcitx.InputMethod1` /
//! `InputContext1`), as fcitx5-gtk talks to it, but with one key per async `ProcessKeyEvent`
//! and the context's signals rather than `ProcessKeyEventBatch`.

use zbus::{message::Message, zvariant::OwnedObjectPath};

use super::{Ime, ImeEvent, RawKey, dbus::Bus, dbus::Post};

const FCITX: &str = "org.fcitx.Fcitx5";
const METHOD: &str = "org.fcitx.Fcitx.InputMethod1";
const CONTEXT: &str = "org.fcitx.Fcitx.InputContext1";

/// Preedit | FormattedPreedit | ClientUnfocusCommit | SurroundingText | KeyEventOrderFix;
/// `SKENE_FCITX_CAPS` overrides (fcitx5-gtk 5.1.1 sends 0xc000000072: it also sets
/// ReportKeyRepeat and ClientSideInputPanel).
fn capabilities() -> u64 {
  std::env::var("SKENE_FCITX_CAPS")
    .ok()
    .and_then(|caps| u64::from_str_radix(caps.trim_start_matches("0x"), 16).ok())
    .unwrap_or((1 << 1) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 37))
}

/// One Fcitx5 input context.
pub struct Fcitx {
  bus: Bus,
  context: OwnedObjectPath,
}

impl Fcitx {
  /// Connects to Fcitx5 on the session bus and makes an input context, its events posted
  /// through `post`.
  pub fn connect(post: Post) -> Result<Self, Box<dyn std::error::Error>> {
    let bus = Bus::connect("fcitx", None, FCITX)?;
    let display = format!("x11:{}", std::env::var("DISPLAY").unwrap_or_default());
    let hints = vec![("display", display.as_str()), ("program", "skene")];
    let reply = bus.conn.call_method(
      Some(FCITX),
      "/org/freedesktop/portal/inputmethod",
      Some(METHOD),
      "CreateInputContext",
      &(hints,),
    )?;
    let (context, _uuid): (OwnedObjectPath, Vec<u8>) = reply.body().deserialize()?;

    bus.conn.call_method(
      Some("org.freedesktop.DBus"),
      "/org/freedesktop/DBus",
      Some("org.freedesktop.DBus"),
      "AddMatch",
      &(format!("type='signal',path='{}'", context.as_str()),),
    )?;
    bus.spawn_reader(context.clone(), decode, post.clone());
    bus.call(&context, CONTEXT, "SetCapability", &(capabilities(),));
    post(ImeEvent::Ready(format!(
      "fcitx context {} caps 0x{:x}",
      context.as_str(),
      capabilities()
    )));

    Ok(Self { bus, context })
  }
}

fn decode(message: &Message, member: &str) -> Vec<ImeEvent> {
  let body = message.body();

  match member {
    "CommitString" => body
      .deserialize::<String>()
      .map(ImeEvent::Commit)
      .into_iter()
      .collect(),
    "UpdateFormattedPreedit" => {
      let Ok((segments, cursor)) = body.deserialize::<(Vec<(String, i32)>, i32)>() else {
        return vec![ImeEvent::Note("bad UpdateFormattedPreedit".into())];
      };
      let text: String = segments.iter().map(|(text, _)| text.as_str()).collect();
      let cursor = u32::try_from(cursor)
        .ok()
        .map(|byte| super::byte_to_char(&text, byte as usize) as u32);

      vec![ImeEvent::Preedit(text, cursor)]
    }
    "DeleteSurroundingText" => body
      .deserialize::<(i32, u32)>()
      .map(|(offset, len)| ImeEvent::DeleteSurrounding(offset, len))
      .into_iter()
      .collect(),
    "ForwardKey" => body
      .deserialize::<(u32, u32, bool)>()
      .map(|(keysym, state, release)| ImeEvent::Forward {
        keysym,
        keycode: None,
        state,
        release,
      })
      .into_iter()
      .collect(),
    "CurrentIM" => vec![ImeEvent::Note(format!(
      "current im {:?}",
      body.deserialize::<(String, String, String)>().ok()
    ))],
    other => vec![ImeEvent::Note(format!("signal {other}"))],
  }
}

impl Ime for Fcitx {
  fn name(&self) -> &'static str {
    "fcitx"
  }

  fn answers_keys(&self) -> bool {
    true
  }

  fn process_key(&mut self, id: u64, key: &RawKey) -> bool {
    self.bus.call_key(
      id,
      &self.context,
      CONTEXT,
      "ProcessKeyEvent",
      &(key.keysym, key.keycode, key.state, key.release, key.time),
    );
    true
  }

  fn focus_in(&mut self) {
    self.bus.call(&self.context, CONTEXT, "FocusIn", &());
  }

  fn set_purpose(&mut self, purpose: super::Purpose) {
    self.bus.call(
      &self.context,
      CONTEXT,
      "SetCapability",
      &(capabilities() | purpose.fcitx(),),
    );
  }

  fn focus_out(&mut self) {
    self.bus.call(&self.context, CONTEXT, "FocusOut", &());
  }

  fn reset(&mut self) {
    self.bus.call(&self.context, CONTEXT, "Reset", &());
  }

  fn set_cursor(&mut self, root: (i32, i32, i32, i32), _window: (i32, i32, i32, i32)) {
    self
      .bus
      .call(&self.context, CONTEXT, "SetCursorRect", &root);
  }

  fn set_surrounding(&mut self, text: &str, cursor: u32, anchor: u32) {
    self.bus.call(
      &self.context,
      CONTEXT,
      "SetSurroundingText",
      &(text, cursor, anchor),
    );
  }
}
