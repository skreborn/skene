//! A screen reader's client of the accessibility bus (AT-SPI over D-Bus, through GIO): what
//! Orca reads of this process's own app.
//!
//! It runs on GTK's thread, in the app's own process: each call's reply is awaited on the
//! default main context (`MainContext::block_on`), which keeps dispatching while it waits, so
//! GTK answers the very requests the call caused. A reader in another process sends the same
//! messages over the same bus.

use std::time::{Duration, Instant};

use gtk::{gio, glib, prelude::*};

/// An object on the accessibility bus: its application's bus name and its path.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Object {
  /// The bus name of the application holding it.
  pub bus: String,
  /// Its path.
  pub path: String,
}

/// AT-SPI's state bits the driver reads.
pub mod state {
  /// `ATSPI_STATE_CHECKED`.
  pub const CHECKED: u32 = 4;
  /// `ATSPI_STATE_EDITABLE`.
  pub const EDITABLE: u32 = 7;
  /// `ATSPI_STATE_ENABLED`.
  pub const ENABLED: u32 = 8;
  /// `ATSPI_STATE_EXPANDED`.
  pub const EXPANDED: u32 = 10;
  /// `ATSPI_STATE_FOCUSABLE`.
  pub const FOCUSABLE: u32 = 11;
  /// `ATSPI_STATE_FOCUSED`.
  pub const FOCUSED: u32 = 12;
  /// `ATSPI_STATE_MULTI_LINE`.
  pub const MULTI_LINE: u32 = 17;
  /// `ATSPI_STATE_PRESSED`.
  pub const PRESSED: u32 = 20;
  /// `ATSPI_STATE_SELECTED`.
  pub const SELECTED: u32 = 23;
  /// `ATSPI_STATE_SENSITIVE`.
  pub const SENSITIVE: u32 = 24;
  /// `ATSPI_STATE_SHOWING`.
  pub const SHOWING: u32 = 25;
  /// `ATSPI_STATE_VISIBLE`.
  pub const VISIBLE: u32 = 30;
  /// `ATSPI_STATE_INDETERMINATE`.
  pub const INDETERMINATE: u32 = 32;
  /// `ATSPI_STATE_REQUIRED`.
  pub const REQUIRED: u32 = 33;
  /// `ATSPI_STATE_INVALID_ENTRY`.
  pub const INVALID_ENTRY: u32 = 36;
  /// `ATSPI_STATE_READ_ONLY`.
  pub const READ_ONLY: u32 = 43;

  /// The names of the states set in `states`, for an outline.
  #[must_use]
  pub fn names(states: u64) -> Vec<&'static str> {
    [
      (CHECKED, "checked"),
      (EDITABLE, "editable"),
      (EXPANDED, "expanded"),
      (FOCUSED, "focused"),
      (INDETERMINATE, "indeterminate"),
      (INVALID_ENTRY, "invalid"),
      (MULTI_LINE, "multi-line"),
      (PRESSED, "pressed"),
      (READ_ONLY, "read-only"),
      (REQUIRED, "required"),
      (SELECTED, "selected"),
    ]
    .into_iter()
    .filter(|(bit, _)| states & (1 << bit) != 0)
    .map(|(_, name)| name)
    .chain((states & (1 << SENSITIVE) == 0).then_some("disabled"))
    .collect()
  }
}

/// One object as a reader finds it while walking the tree.
#[derive(Clone, PartialEq, Debug)]
pub struct Item {
  /// The object.
  pub object: Object,
  /// Its depth below the root of the walk.
  pub depth: usize,
  /// Its role, by AT-SPI's name for it (`"push button"`).
  pub role: String,
  /// Its accessible name.
  pub name: String,
  /// Its states, as AT-SPI's bit set.
  pub states: u64,
}

const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
const ROOT: &str = "/org/a11y/atspi/accessible/root";
const REGISTRY: &str = "org.a11y.atspi.Registry";

/// A failed call, as a message.
pub type Result<T> = std::result::Result<T, String>;

/// A connection to the accessibility bus, and this process's application on it.
pub struct Atspi {
  connection: gio::DBusConnection,
  /// This process's application.
  pub application: Object,
}

/// Runs `future` to completion on the default main context, which keeps dispatching while it
/// waits.
fn block<F: std::future::Future>(future: F) -> F::Output {
  glib::MainContext::default().block_on(future)
}

impl Atspi {
  /// Connects to the accessibility bus and finds this process's application among the
  /// registry's, waiting up to `timeout` for it to register.
  ///
  /// # Errors
  ///
  /// Where there is no accessibility bus, or the application never registers on it.
  pub fn connect(timeout: Duration) -> Result<Self> {
    let started = Instant::now();
    let pid = std::process::id();

    block(async move {
      let session = gio::bus_get_future(gio::BusType::Session)
        .await
        .map_err(|error| format!("no session bus: {error}"))?;
      let address = session
        .call_future(
          Some("org.a11y.Bus"),
          "/org/a11y/bus",
          "org.a11y.Bus",
          "GetAddress",
          None,
          None,
          gio::DBusCallFlags::NONE,
          5000,
        )
        .await
        .map_err(|error| format!("no accessibility bus: {error}"))?
        .child_value(0)
        .str()
        .unwrap_or_default()
        .to_owned();
      let connection = gio::DBusConnection::for_address_future(
        &address,
        gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
          | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
        None,
      )
      .await
      .map_err(|error| format!("cannot connect to the accessibility bus: {error}"))?;

      loop {
        let applications = call(
          &connection,
          &Object {
            bus: REGISTRY.to_owned(),
            path: ROOT.to_owned(),
          },
          ACCESSIBLE,
          "GetChildren",
          None,
        )
        .await?;

        for application in references(&applications.child_value(0)) {
          let owner = connection
            .call_future(
              Some("org.freedesktop.DBus"),
              "/org/freedesktop/DBus",
              "org.freedesktop.DBus",
              "GetConnectionUnixProcessID",
              Some(&(application.bus.as_str(),).to_variant()),
              None,
              gio::DBusCallFlags::NONE,
              5000,
            )
            .await
            .ok()
            .and_then(|reply| reply.child_value(0).get::<u32>());

          if owner == Some(pid) {
            return Ok(Self {
              connection,
              application: Object {
                bus: application.bus,
                path: ROOT.to_owned(),
              },
            });
          }
        }

        if started.elapsed() > timeout {
          return Err(format!(
            "this process (pid {pid}) never registered on the accessibility bus"
          ));
        }

        glib::timeout_future(Duration::from_millis(10)).await;
      }
    })
  }

  /// Calls `method` of `interface` on `object`, waiting for the reply.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn call(
    &self,
    object: &Object,
    interface: &str,
    method: &str,
    arguments: Option<glib::Variant>,
  ) -> Result<glib::Variant> {
    block(call(&self.connection, object, interface, method, arguments))
  }

  /// The property `name` of `interface` on `object`.
  fn property(&self, object: &Object, interface: &str, name: &str) -> Result<glib::Variant> {
    block(property(&self.connection, object, interface, name))
  }

  /// The application's top-level object named `name` (a window, by its title).
  ///
  /// # Errors
  ///
  /// Where the application cannot be read.
  pub fn toplevel(&self, name: &str) -> Result<Option<Object>> {
    let children = self.children(&self.application)?;

    for child in children {
      if self.name(&child)? == name {
        return Ok(Some(child));
      }
    }

    Ok(None)
  }

  /// `object`'s children.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn children(&self, object: &Object) -> Result<Vec<Object>> {
    Ok(references(
      &self
        .call(object, ACCESSIBLE, "GetChildren", None)?
        .child_value(0),
    ))
  }

  /// The tree below `root`, `root` included, depth first: each level's objects read at once.
  ///
  /// # Errors
  ///
  /// Where a call fails.
  pub fn tree(&self, root: &Object) -> Result<Vec<Item>> {
    let context = glib::MainContext::default();
    let mut levels: Vec<Vec<(Item, Vec<Object>)>> = Vec::new();
    let mut next = vec![root.clone()];
    let mut depth = 0;

    while !next.is_empty() {
      let handles: Vec<_> = next
        .drain(..)
        .map(|object| {
          let connection = self.connection.clone();

          context.spawn_local(async move { item(&connection, object, depth).await })
        })
        .collect();
      let level = block(async move {
        let mut level = Vec::new();

        for handle in handles {
          level.push(handle.await.map_err(|error| error.to_string())??);
        }

        Ok::<_, String>(level)
      })?;

      next = level
        .iter()
        .flat_map(|(_, children)| children.iter().cloned())
        .collect();
      levels.push(level);
      depth += 1;
    }

    let mut by_path = std::collections::HashMap::new();

    for (item, children) in levels.into_iter().flatten() {
      by_path.insert(item.object.path.clone(), (item, children));
    }

    let mut items = Vec::new();
    let mut stack = vec![root.path.clone()];

    while let Some(path) = stack.pop() {
      if let Some((item, children)) = by_path.remove(&path) {
        stack.extend(children.into_iter().rev().map(|child| child.path));
        items.push(item);
      }
    }

    Ok(items)
  }

  /// `object`'s accessible name.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn name(&self, object: &Object) -> Result<String> {
    Ok(
      self
        .property(object, ACCESSIBLE, "Name")?
        .str()
        .unwrap_or_default()
        .to_owned(),
    )
  }

  /// `object`'s description.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn description(&self, object: &Object) -> Result<String> {
    Ok(
      self
        .property(object, ACCESSIBLE, "Description")?
        .str()
        .unwrap_or_default()
        .to_owned(),
    )
  }

  /// `object`'s role, by AT-SPI's name for it.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn role(&self, object: &Object) -> Result<String> {
    Ok(
      self
        .call(object, ACCESSIBLE, "GetRoleName", None)?
        .child_value(0)
        .str()
        .unwrap_or_default()
        .to_owned(),
    )
  }

  /// `object`'s states, as AT-SPI's bit set.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn states(&self, object: &Object) -> Result<u64> {
    Ok(bits(
      &self
        .call(object, ACCESSIBLE, "GetState", None)?
        .child_value(0),
    ))
  }

  /// The interfaces `object` implements.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn interfaces(&self, object: &Object) -> Result<Vec<String>> {
    Ok(
      self
        .call(object, ACCESSIBLE, "GetInterfaces", None)?
        .child_value(0)
        .iter()
        .filter_map(|name| name.str().map(str::to_owned))
        .collect(),
    )
  }

  /// `object`'s current value, where it has one (`org.a11y.atspi.Value`).
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn value(&self, object: &Object) -> Result<f64> {
    self
      .property(object, "org.a11y.atspi.Value", "CurrentValue")?
      .get::<f64>()
      .ok_or_else(|| "the value is not a number".to_owned())
  }

  /// `object`'s whole text (`org.a11y.atspi.Text`).
  ///
  /// # Errors
  ///
  /// Where it has no text.
  pub fn text(&self, object: &Object) -> Result<String> {
    let count = self
      .property(object, "org.a11y.atspi.Text", "CharacterCount")?
      .get::<i32>()
      .unwrap_or(0);

    Ok(
      self
        .call(
          object,
          "org.a11y.atspi.Text",
          "GetText",
          Some((0i32, count).to_variant()),
        )?
        .child_value(0)
        .str()
        .unwrap_or_default()
        .to_owned(),
    )
  }

  /// `object`'s caret and first selection, in characters: `(caret, start, end)`.
  ///
  /// # Errors
  ///
  /// Where it has no text.
  pub fn caret_and_selection(&self, object: &Object) -> Result<(i32, i32, i32)> {
    let caret = self
      .property(object, "org.a11y.atspi.Text", "CaretOffset")?
      .get::<i32>()
      .unwrap_or(-1);
    let selections = self
      .call(object, "org.a11y.atspi.Text", "GetNSelections", None)?
      .child_value(0)
      .get::<i32>()
      .unwrap_or(0);

    if selections == 0 {
      return Ok((caret, caret, caret));
    }

    let selection = self.call(
      object,
      "org.a11y.atspi.Text",
      "GetSelection",
      Some((0i32,).to_variant()),
    )?;

    Ok((
      caret,
      selection.child_value(0).get::<i32>().unwrap_or(caret),
      selection.child_value(1).get::<i32>().unwrap_or(caret),
    ))
  }

  /// `object`'s extents relative to its window: `(x, y, width, height)`.
  ///
  /// # Errors
  ///
  /// Where it is not a component.
  pub fn extents(&self, object: &Object) -> Result<(i32, i32, i32, i32)> {
    let reply = self.call(
      object,
      "org.a11y.atspi.Component",
      "GetExtents",
      Some((1u32,).to_variant()),
    )?;
    let extents = reply.child_value(0);
    let at = |index| extents.child_value(index).get::<i32>().unwrap_or(0);

    Ok((at(0), at(1), at(2), at(3)))
  }
}

/// Calls `method` on `object` over `connection`.
async fn call(
  connection: &gio::DBusConnection,
  object: &Object,
  interface: &str,
  method: &str,
  arguments: Option<glib::Variant>,
) -> Result<glib::Variant> {
  connection
    .call_future(
      Some(&object.bus),
      &object.path,
      interface,
      method,
      arguments.as_ref(),
      None,
      gio::DBusCallFlags::NONE,
      5000,
    )
    .await
    .map_err(|error| format!("{interface}.{method} on {}: {error}", object.path))
}

/// The property `name` of `interface` on `object`.
async fn property(
  connection: &gio::DBusConnection,
  object: &Object,
  interface: &str,
  name: &str,
) -> Result<glib::Variant> {
  let reply = call(
    connection,
    object,
    "org.freedesktop.DBus.Properties",
    "Get",
    Some((interface, name).to_variant()),
  )
  .await?;

  reply
    .child_value(0)
    .as_variant()
    .ok_or_else(|| format!("{interface}.{name} is not a variant"))
}

/// `object` read for a walk: its item and its children.
async fn item(
  connection: &gio::DBusConnection,
  object: Object,
  depth: usize,
) -> Result<(Item, Vec<Object>)> {
  let role = call(connection, &object, ACCESSIBLE, "GetRoleName", None).await?;
  let name = property(connection, &object, ACCESSIBLE, "Name").await?;
  let states = call(connection, &object, ACCESSIBLE, "GetState", None).await?;
  let children = call(connection, &object, ACCESSIBLE, "GetChildren", None).await?;

  Ok((
    Item {
      depth,
      role: role.child_value(0).str().unwrap_or_default().to_owned(),
      name: name.str().unwrap_or_default().to_owned(),
      states: bits(&states.child_value(0)),
      object,
    },
    references(&children.child_value(0)),
  ))
}

/// The objects of an `a(so)` array.
fn references(array: &glib::Variant) -> Vec<Object> {
  array
    .iter()
    .map(|reference| Object {
      bus: reference
        .child_value(0)
        .str()
        .unwrap_or_default()
        .to_owned(),
      path: reference
        .child_value(1)
        .str()
        .unwrap_or_default()
        .to_owned(),
    })
    .collect()
}

/// AT-SPI's state words as one bit set.
fn bits(words: &glib::Variant) -> u64 {
  words
    .iter()
    .filter_map(|word| word.get::<u32>())
    .enumerate()
    .fold(0, |bits, (index, word)| {
      bits | (u64::from(word) << (32 * index))
    })
}
