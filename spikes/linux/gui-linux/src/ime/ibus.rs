//! IBus over D-Bus, as GTK's `im-ibus` module talks to it: IBus's own bus (from
//! `IBUS_ADDRESS` or the address file ibus-daemon writes per display), one input context, keys
//! through async `ProcessKeyEvent`, and the context's signals.

use std::sync::Mutex;

use zbus::{
  message::Message,
  zvariant::{OwnedObjectPath, OwnedValue, StructureBuilder, Value},
};

use super::{Ime, ImeEvent, RawKey, dbus::Bus, dbus::Post};

const IBUS: &str = "org.freedesktop.IBus";
const CONTEXT: &str = "org.freedesktop.IBus.InputContext";
/// IBUS_CAP_PREEDIT_TEXT | IBUS_CAP_FOCUS | IBUS_CAP_SURROUNDING_TEXT, as GTK sets them.
const CAPABILITIES: u32 = 1 | 8 | 32;
const RELEASE_MASK: u32 = 1 << 30;

/// The address of the IBus bus for this display: `IBUS_ADDRESS`, else the file
/// `$XDG_CONFIG_HOME/ibus/bus/<machine-id>-<host>-<display>` whose daemon is alive.
pub fn address() -> Option<String> {
  if let Ok(address) = std::env::var("IBUS_ADDRESS") {
    return Some(address);
  }

  let display = std::env::var("DISPLAY").ok()?;
  let (host, rest) = display.split_once(':')?;
  let number = rest.split('.').next()?;
  let host = if host.is_empty() { "unix" } else { host };
  let machine = std::fs::read_to_string("/var/lib/dbus/machine-id")
    .or_else(|_| std::fs::read_to_string("/etc/machine-id"))
    .ok()?;
  let config = std::env::var("XDG_CONFIG_HOME")
    .unwrap_or_else(|_| format!("{}/.config", std::env::var("HOME").unwrap_or_default()));
  let file = format!("{config}/ibus/bus/{}-{host}-{number}", machine.trim());
  let contents = std::fs::read_to_string(file).ok()?;
  let field = |name: &str| {
    contents
      .lines()
      .find_map(|line| line.strip_prefix(name))
      .map(str::to_owned)
  };
  let pid: i32 = field("IBUS_DAEMON_PID=")?.parse().ok()?;

  // SAFETY: signal 0 only checks that the process exists.
  if unsafe { libc::kill(pid, 0) } != 0 {
    return None;
  }

  field("IBUS_ADDRESS=")
}

/// The text inside a serialized `IBusText` (`(sa{sv}sv)`).
#[must_use]
pub fn ibus_text(value: &Value<'_>) -> Option<String> {
  let value = match value {
    Value::Value(inner) => inner,
    other => other,
  };
  let Value::Structure(structure) = value else {
    return None;
  };

  match structure.fields().get(2)? {
    Value::Str(text) => Some(text.to_string()),
    _ => None,
  }
}

/// An `IBusText` holding `text` with no attributes.
#[must_use]
pub fn to_ibus_text(text: &str) -> Value<'static> {
  let attributes = StructureBuilder::new()
    .add_field("IBusAttrList")
    .add_field(std::collections::HashMap::<String, Value<'static>>::new())
    .add_field(Vec::<Value<'static>>::new())
    .build()
    .unwrap();
  let text = StructureBuilder::new()
    .add_field("IBusText")
    .add_field(std::collections::HashMap::<String, Value<'static>>::new())
    .add_field(text.to_owned())
    .append_field(Value::Value(Box::new(Value::Structure(attributes))))
    .build()
    .unwrap();

  Value::Structure(text)
}

/// Whether a secret's keys stay away from IBus, as IBus's GTK module decides it: where
/// `IBUS_DISCARD_PASSWORD` is set to anything but empty, `0` or `false`, or where a name in
/// the comma-separated `IBUS_DISCARD_PASSWORD_APPS` occurs in the program's name.
#[must_use]
pub fn discards_passwords() -> bool {
  let set = std::env::var("IBUS_DISCARD_PASSWORD")
    .is_ok_and(|value| !matches!(value.as_str(), "" | "0" | "false" | "False" | "FALSE"));
  let program = std::env::args_os()
    .next()
    .and_then(|path| {
      std::path::Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
    })
    .unwrap_or_default();
  let listed = std::env::var("IBUS_DISCARD_PASSWORD_APPS").is_ok_and(|apps| {
    apps
      .split(',')
      .any(|app| !app.is_empty() && program.contains(app))
  });

  set || listed
}

/// One IBus input context.
pub struct IBus {
  bus: Bus,
  context: OwnedObjectPath,
  discards_passwords: bool,
}

impl IBus {
  /// Connects to IBus's own bus and makes an input context, its events posted through
  /// `post`.
  pub fn connect(post: Post) -> Result<Self, Box<dyn std::error::Error>> {
    let address = address().ok_or("no IBus address")?;
    let bus = Bus::connect("ibus", Some(&address), IBUS)?;
    let reply = bus.conn.call_method(
      Some(IBUS),
      "/org/freedesktop/IBus",
      Some(IBUS),
      "CreateInputContext",
      &("skene",),
    )?;
    let context: OwnedObjectPath = reply.body().deserialize()?;
    let preedit = Mutex::new((String::new(), 0u32, false));

    bus.spawn_reader(
      context.clone(),
      move |message, member| decode(message, member, &preedit),
      post.clone(),
    );

    let set = |name: &str, value: Value<'_>| {
      bus.call(
        &context,
        "org.freedesktop.DBus.Properties",
        "Set",
        &(CONTEXT, name, value),
      );
    };

    if std::env::var_os("SKENE_IBUS_NO_CLIENT_COMMIT").is_none() {
      set(
        "ClientCommitPreedit",
        Value::Structure(StructureBuilder::new().add_field(true).build().unwrap()),
      );
    }

    set(
      "ContentType",
      Value::Structure(
        StructureBuilder::new()
          .add_field(0u32)
          .add_field(0u32)
          .build()
          .unwrap(),
      ),
    );
    bus.call(&context, CONTEXT, "SetCapabilities", &(CAPABILITIES,));
    post(ImeEvent::Ready(format!(
      "ibus {address} context {}",
      context.as_str()
    )));

    Ok(Self {
      bus,
      context,
      discards_passwords: discards_passwords(),
    })
  }
}

fn decode(message: &Message, member: &str, preedit: &Mutex<(String, u32, bool)>) -> Vec<ImeEvent> {
  let body = message.body();
  let show = |preedit: &(String, u32, bool)| {
    if preedit.2 && !preedit.0.is_empty() {
      ImeEvent::Preedit(preedit.0.clone(), Some(preedit.1))
    } else {
      ImeEvent::Preedit(String::new(), None)
    }
  };

  match member {
    "CommitText" => body
      .deserialize::<OwnedValue>()
      .ok()
      .and_then(|value| ibus_text(&value))
      .map(ImeEvent::Commit)
      .into_iter()
      .collect(),
    "UpdatePreeditText" | "UpdatePreeditTextWithMode" => {
      let parsed = if member == "UpdatePreeditText" {
        body.deserialize::<(OwnedValue, u32, bool)>().ok()
      } else {
        body
          .deserialize::<(OwnedValue, u32, bool, u32)>()
          .ok()
          .map(|(text, cursor, visible, _mode)| (text, cursor, visible))
      };
      let Some((text, cursor, visible)) = parsed else {
        return vec![ImeEvent::Note(format!("bad {member}"))];
      };
      let mut preedit = preedit.lock().unwrap();

      *preedit = (ibus_text(&text).unwrap_or_default(), cursor, visible);
      vec![show(&preedit)]
    }
    "ShowPreeditText" | "HidePreeditText" => {
      let mut preedit = preedit.lock().unwrap();

      preedit.2 = member == "ShowPreeditText";
      vec![show(&preedit)]
    }
    "DeleteSurroundingText" => body
      .deserialize::<(i32, u32)>()
      .map(|(offset, len)| ImeEvent::DeleteSurrounding(offset, len))
      .into_iter()
      .collect(),
    "ForwardKeyEvent" => body
      .deserialize::<(u32, u32, u32)>()
      .map(|(keysym, keycode, state)| ImeEvent::Forward {
        keysym,
        keycode: (keycode != 0).then_some(keycode + 8),
        state: state & !RELEASE_MASK,
        release: state & RELEASE_MASK != 0,
      })
      .into_iter()
      .collect(),
    "RequireSurroundingText" => vec![ImeEvent::RequireSurrounding],
    other => vec![ImeEvent::Note(format!("signal {other}"))],
  }
}

impl Ime for IBus {
  fn name(&self) -> &'static str {
    "ibus"
  }

  fn answers_keys(&self) -> bool {
    true
  }

  fn takes_secrets(&self) -> bool {
    !self.discards_passwords
  }

  fn process_key(&mut self, id: u64, key: &RawKey) -> bool {
    let state = key.state | if key.release { RELEASE_MASK } else { 0 };

    self.bus.call_key(
      id,
      &self.context,
      CONTEXT,
      "ProcessKeyEvent",
      &(key.keysym, key.keycode - 8, state),
    );
    true
  }

  fn focus_in(&mut self) {
    self.bus.call(&self.context, CONTEXT, "FocusIn", &());
  }

  fn set_purpose(&mut self, purpose: super::Purpose) {
    self.bus.call(
      &self.context,
      "org.freedesktop.DBus.Properties",
      "Set",
      &(
        CONTEXT,
        "ContentType",
        Value::Structure(zbus::zvariant::Structure::from((
          purpose.ibus(),
          purpose.ibus_hints(),
        ))),
      ),
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
      .call(&self.context, CONTEXT, "SetCursorLocation", &root);
  }

  fn set_surrounding(&mut self, text: &str, cursor: u32, anchor: u32) {
    self.bus.call(
      &self.context,
      CONTEXT,
      "SetSurroundingText",
      &(Value::Value(Box::new(to_ibus_text(text))), cursor, anchor),
    );
  }
}
