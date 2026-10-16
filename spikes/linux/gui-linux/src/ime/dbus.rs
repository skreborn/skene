//! What the IBus and Fcitx5 clients share: one zbus connection, a reader thread that hands
//! every message on in arrival order (signals and key replies interleaved as sent), and
//! fire-and-forget calls whose replies the reader matches by serial.

use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
  time::Instant,
};

use zbus::{
  blocking::{Connection, MessageIterator},
  message::{Message, Type},
  zvariant::{DynamicType, ObjectPath, OwnedObjectPath, OwnedValue, Value},
};

use super::ImeEvent;

/// Posts an event to the event loop (wakes it).
pub type Post = Arc<dyn Fn(ImeEvent) + Send + Sync>;

/// A D-Bus connection to an input method, closed when it drops: its reader then ends.
pub struct Bus {
  /// The connection.
  pub conn: Connection,
  /// The input method's bus name.
  pub dest: String,
  /// Key calls awaiting their reply: serial -> (key id, sent).
  pending: Arc<Mutex<HashMap<u32, (u64, Instant)>>>,
  /// The input method's name, for the trace.
  name: &'static str,
}

/// The header fields the clients read, parsed from the raw message: ibus-daemon sends its
/// own messages from `org.freedesktop.DBus`, which is not a unique name, and zbus's
/// `Message::header` panics on such a sender.
pub struct Fields {
  /// The object path.
  pub path: Option<String>,
  /// The member (method or signal).
  pub member: Option<String>,
  /// The error's name.
  pub error_name: Option<String>,
  /// The serial of the call a reply answers.
  pub reply_serial: Option<u32>,
}

impl Fields {
  /// The fields of `message`.
  pub fn of(message: &Message) -> Self {
    let mut fields = Self {
      path: None,
      member: None,
      error_name: None,
      reply_serial: None,
    };
    let parsed = message
      .data()
      .deserialize::<(u8, u8, u8, u8, u32, u32, Vec<(u8, OwnedValue)>)>();

    if let Ok(((.., list), _)) = parsed {
      for (code, value) in list {
        match (code, &*value) {
          (1, Value::ObjectPath(path)) => fields.path = Some(path.to_string()),
          (3, Value::Str(member)) => fields.member = Some(member.to_string()),
          (4, Value::Str(name)) => fields.error_name = Some(name.to_string()),
          (5, Value::U32(serial)) => fields.reply_serial = Some(*serial),
          _ => {}
        }
      }
    }

    fields
  }
}

/// `address` with only the keys zbus accepts: fcitx5's IBus frontend writes the session bus
/// address with a `fcitx_random_string=` key, which libdbus and GDBus ignore and zbus rejects.
pub fn sanitize(address: &str) -> String {
  address
    .split(';')
    .map(|entry| {
      let Some((transport, keys)) = entry.split_once(':') else {
        return entry.to_owned();
      };
      let kept: Vec<&str> = keys
        .split(',')
        .filter(|pair| {
          matches!(
            pair.split('=').next(),
            Some("path" | "abstract" | "guid" | "dir" | "tmpdir" | "host" | "port" | "family")
          )
        })
        .collect();

      format!("{transport}:{}", kept.join(","))
    })
    .collect::<Vec<_>>()
    .join(";")
}

/// Whether `name` has an owner on the session bus (never activates it).
pub fn session_has_name(name: &str) -> bool {
  let Ok(conn) = Connection::session() else {
    return false;
  };

  conn
    .call_method(
      Some("org.freedesktop.DBus"),
      "/org/freedesktop/DBus",
      Some("org.freedesktop.DBus"),
      "NameHasOwner",
      &(name,),
    )
    .and_then(|reply| reply.body().deserialize::<bool>())
    .unwrap_or(false)
}

impl Drop for Bus {
  fn drop(&mut self) {
    let _ = self.conn.clone().close();
  }
}

impl Bus {
  /// Connects to the session bus, or to `address` (IBus's own bus), and fails without
  /// D-Bus-activating anything when `dest` has no owner there.
  pub fn connect(name: &'static str, address: Option<&str>, dest: &str) -> zbus::Result<Self> {
    let conn = match address {
      Some(address) => {
        zbus::blocking::connection::Builder::address(sanitize(address).as_str())?.build()?
      }
      None => Connection::session()?,
    };
    let owned = conn
      .call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "NameHasOwner",
        &(dest,),
      )?
      .body()
      .deserialize::<bool>()?;

    if !owned {
      return Err(zbus::Error::Failure(format!("{dest} has no owner")));
    }

    Ok(Self {
      conn,
      dest: dest.into(),
      pending: Arc::default(),
      name,
    })
  }

  /// Starts the reader: replies to key calls become `KeyDone`, and every signal on `path` goes
  /// through `decode`, in the order the connection received them.
  pub fn spawn_reader(
    &self,
    path: OwnedObjectPath,
    decode: impl Fn(&Message, &str) -> Vec<ImeEvent> + Send + 'static,
    post: Post,
  ) {
    let iterator = MessageIterator::from(self.conn.clone());
    let pending = self.pending.clone();
    let name = self.name;

    std::thread::Builder::new()
      .name(format!("{name}-reader"))
      .spawn(move || {
        for message in iterator {
          let Ok(message) = message else {
            continue;
          };
          let header = Fields::of(&message);

          match message.message_type() {
            Type::MethodReturn | Type::Error => {
              let Some(serial) = header.reply_serial else {
                continue;
              };
              let Some((id, sent)) = pending.lock().unwrap().remove(&serial) else {
                continue;
              };
              let handled = message.message_type() == Type::MethodReturn
                && message.body().deserialize::<bool>().unwrap_or(false);

              if message.message_type() == Type::Error {
                post(ImeEvent::Note(format!(
                  "key {id} error {:?}",
                  header.error_name
                )));
              }

              super::log_latency(name, "reply", sent);
              post(ImeEvent::KeyDone { id, handled });
            }
            Type::Signal if header.path.as_deref() == Some(path.as_str()) => {
              for event in decode(&message, header.member.as_deref().unwrap_or_default()) {
                post(event);
              }
            }
            _ => {}
          }
        }

        post(ImeEvent::Note("connection closed".into()));
      })
      .expect("reader thread");
  }

  fn message<B>(
    &self,
    path: &ObjectPath<'_>,
    interface: &str,
    member: &str,
    body: &B,
  ) -> zbus::Result<Message>
  where
    B: serde::Serialize + DynamicType,
  {
    Message::method_call(path.clone(), member)?
      .destination(self.dest.as_str())?
      .interface(interface)?
      .build(body)
  }

  /// Sends a call without waiting for its reply.
  pub fn call<B>(&self, path: &ObjectPath<'_>, interface: &str, member: &str, body: &B)
  where
    B: serde::Serialize + DynamicType,
  {
    let result = self
      .message(path, interface, member, body)
      .and_then(|message| self.conn.send(&message));

    if let Err(error) = result {
      crate::trace::log(|| format!("[{}] {member} failed: {error}", self.name));
    }
  }

  /// Sends a key call whose boolean reply becomes `KeyDone { id }`.
  pub fn call_key<B>(&self, id: u64, path: &ObjectPath<'_>, interface: &str, member: &str, body: &B)
  where
    B: serde::Serialize + DynamicType,
  {
    match self.message(path, interface, member, body) {
      Ok(message) => {
        self.pending.lock().unwrap().insert(
          message.primary_header().serial_num().get(),
          (id, Instant::now()),
        );

        if let Err(error) = self.conn.send(&message) {
          crate::trace::log(|| format!("[{}] {member} failed: {error}", self.name));
        }
      }
      Err(error) => crate::trace::log(|| format!("[{}] {member} failed: {error}", self.name)),
    }
  }
}
