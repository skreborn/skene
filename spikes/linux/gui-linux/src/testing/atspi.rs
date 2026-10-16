//! A screen reader's client of the accessibility bus: AT-SPI over D-Bus, what Orca reads of
//! the process's own app.
//!
//! The app answers from the accessibility layer's own thread (AccessKit's), so a call made on
//! the window's thread is answered while that thread waits; a reader in another process sends
//! the same messages over the same bus.

use std::time::{Duration, Instant};

use zbus::{
  blocking::Connection,
  zvariant::{OwnedObjectPath, OwnedValue},
};

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
const TEXT: &str = "org.a11y.atspi.Text";
const ROOT: &str = "/org/a11y/atspi/accessible/root";
const REGISTRY: &str = "org.a11y.atspi.Registry";

/// AT-SPI's coordinate type for the screen.
pub const SCREEN: u32 = 0;
/// AT-SPI's coordinate type for the window.
pub const WINDOW: u32 = 1;

/// A failed call, as a message.
pub type Result<T> = std::result::Result<T, String>;

/// Turns on the accessibility bus's `IsEnabled`, as a screen reader does when it starts, so
/// applications report to it.
///
/// # Errors
///
/// Where there is no session bus or no accessibility bus launcher on it.
pub fn enable() -> Result<()> {
  let session = Connection::session().map_err(|error| format!("no session bus: {error}"))?;

  session
    .call_method(
      Some("org.a11y.Bus"),
      "/org/a11y/bus",
      Some("org.freedesktop.DBus.Properties"),
      "Set",
      &(
        "org.a11y.Status",
        "IsEnabled",
        zbus::zvariant::Value::from(true),
      ),
    )
    .map_err(|error| format!("cannot turn the accessibility bus on: {error}"))?;

  Ok(())
}

/// A connection to the accessibility bus, and this process's application on it.
pub struct Atspi {
  connection: Connection,
  /// This process's application.
  pub application: Object,
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
    let session = Connection::session().map_err(|error| format!("no session bus: {error}"))?;
    let address: String = session
      .call_method(
        Some("org.a11y.Bus"),
        "/org/a11y/bus",
        Some("org.a11y.Bus"),
        "GetAddress",
        &(),
      )
      .map_err(|error| format!("no accessibility bus: {error}"))?
      .body()
      .deserialize()
      .map_err(|error| error.to_string())?;
    let connection = zbus::blocking::connection::Builder::address(address.as_str())
      .and_then(|builder| builder.method_timeout(Duration::from_secs(5)).build())
      .map_err(|error| format!("cannot connect to the accessibility bus: {error}"))?;
    let registry = Object {
      bus: REGISTRY.to_owned(),
      path: ROOT.to_owned(),
    };
    let mut atspi = Self {
      connection,
      application: registry.clone(),
    };

    loop {
      for application in atspi.children(&registry).unwrap_or_default() {
        let owner: Option<u32> = atspi
          .connection
          .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetConnectionUnixProcessID",
            &(application.bus.as_str(),),
          )
          .ok()
          .and_then(|reply| reply.body().deserialize().ok());

        if owner == Some(pid) {
          atspi.application = Object {
            bus: application.bus,
            path: ROOT.to_owned(),
          };

          return Ok(atspi);
        }
      }

      if started.elapsed() > timeout {
        return Err(format!(
          "this process (pid {pid}) never registered on the accessibility bus"
        ));
      }

      std::thread::sleep(Duration::from_millis(10));
    }
  }

  /// Calls `method` of `interface` on `object` with `body`, its reply deserialized.
  ///
  /// # Errors
  ///
  /// Where the call fails or its reply is not a `T`.
  pub fn call<B, T>(&self, object: &Object, interface: &str, method: &str, body: &B) -> Result<T>
  where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    T: for<'d> serde::Deserialize<'d> + zbus::zvariant::Type,
  {
    self
      .connection
      .call_method(
        Some(object.bus.as_str()),
        object.path.as_str(),
        Some(interface),
        method,
        body,
      )
      .map_err(|error| format!("{interface}.{method} on {}: {error}", object.path))?
      .body()
      .deserialize()
      .map_err(|error| format!("{interface}.{method} on {}: {error}", object.path))
  }

  /// The property `name` of `interface` on `object`.
  fn property(&self, object: &Object, interface: &str, name: &str) -> Result<OwnedValue> {
    self.call(
      object,
      "org.freedesktop.DBus.Properties",
      "Get",
      &(interface, name),
    )
  }

  /// The application's top-level object named `name` (a window, by its title).
  ///
  /// # Errors
  ///
  /// Where the application cannot be read.
  pub fn toplevel(&self, name: &str) -> Result<Option<Object>> {
    for child in self.children(&self.application)? {
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
    let children: Vec<(String, OwnedObjectPath)> =
      self.call(object, ACCESSIBLE, "GetChildren", &())?;

    Ok(
      children
        .into_iter()
        .map(|(bus, path)| Object {
          bus,
          path: path.to_string(),
        })
        .collect(),
    )
  }

  /// The tree below `root`, `root` included, depth first.
  ///
  /// # Errors
  ///
  /// Where a call fails.
  pub fn tree(&self, root: &Object) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    let mut stack = vec![(root.clone(), 0)];

    while let Some((object, depth)) = stack.pop() {
      let children = self.children(&object)?;

      items.push(Item {
        role: self.role(&object)?,
        name: self.name(&object)?,
        states: self.states(&object)?,
        depth,
        object,
      });
      stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
    }

    Ok(items)
  }

  /// `object`'s accessible name.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn name(&self, object: &Object) -> Result<String> {
    String::try_from(self.property(object, ACCESSIBLE, "Name")?).map_err(|error| error.to_string())
  }

  /// `object`'s description.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn description(&self, object: &Object) -> Result<String> {
    String::try_from(self.property(object, ACCESSIBLE, "Description")?)
      .map_err(|error| error.to_string())
  }

  /// `object`'s role, by AT-SPI's name for it (libatspi's `atspi_role_get_name`).
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn role(&self, object: &Object) -> Result<String> {
    let role: u32 = self.call(object, ACCESSIBLE, "GetRole", &())?;

    Ok(role_name(role).to_owned())
  }

  /// `object`'s states, as AT-SPI's bit set.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn states(&self, object: &Object) -> Result<u64> {
    let words: Vec<u32> = self.call(object, ACCESSIBLE, "GetState", &())?;

    Ok(
      words
        .into_iter()
        .enumerate()
        .fold(0, |bits, (index, word)| {
          bits | (u64::from(word) << (32 * index))
        }),
    )
  }

  /// The interfaces `object` implements.
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn interfaces(&self, object: &Object) -> Result<Vec<String>> {
    self.call(object, ACCESSIBLE, "GetInterfaces", &())
  }

  /// `object`'s current value, where it has one (`org.a11y.atspi.Value`).
  ///
  /// # Errors
  ///
  /// Where the call fails.
  pub fn value(&self, object: &Object) -> Result<f64> {
    f64::try_from(self.property(object, "org.a11y.atspi.Value", "CurrentValue")?)
      .map_err(|error| error.to_string())
  }

  /// `object`'s whole text (`org.a11y.atspi.Text`).
  ///
  /// # Errors
  ///
  /// Where it has no text.
  pub fn text(&self, object: &Object) -> Result<String> {
    let count = i32::try_from(self.property(object, TEXT, "CharacterCount")?).unwrap_or(0);

    self.call(object, TEXT, "GetText", &(0i32, count))
  }

  /// `object`'s caret and first selection, in characters: `(caret, start, end)`.
  ///
  /// # Errors
  ///
  /// Where it has no text.
  pub fn caret_and_selection(&self, object: &Object) -> Result<(i32, i32, i32)> {
    let caret = i32::try_from(self.property(object, TEXT, "CaretOffset")?).unwrap_or(-1);
    let selections: i32 = self.call(object, TEXT, "GetNSelections", &())?;

    if selections == 0 {
      return Ok((caret, caret, caret));
    }

    let (start, end): (i32, i32) = self.call(object, TEXT, "GetSelection", &(0i32,))?;

    Ok((caret, start, end))
  }

  /// `object`'s extents in coordinates of `kind` ([`SCREEN`] or [`WINDOW`]):
  /// `(x, y, width, height)`.
  ///
  /// # Errors
  ///
  /// Where it is not a component.
  pub fn extents(&self, object: &Object, kind: u32) -> Result<(i32, i32, i32, i32)> {
    self.call(object, "org.a11y.atspi.Component", "GetExtents", &(kind,))
  }

  /// Asks for `request` of `object` as a reader does, from a thread of its own, so the
  /// application's loop on this thread turns while the reader's call waits on it: a click as
  /// AT-SPI's first action (`Action.DoAction`), a value through `Value.CurrentValue`, focus
  /// through `Component.GrabFocus`.
  pub fn request(
    &self,
    object: &Object,
    request: skene_foundation::seam::control::AccessRequest,
  ) -> std::thread::JoinHandle<Result<()>> {
    use skene_foundation::seam::control::AccessRequest;

    let reader = Self {
      connection: self.connection.clone(),
      application: self.application.clone(),
    };
    let object = object.clone();

    std::thread::spawn(move || {
      let set = |value: f64| -> Result<()> {
        reader.call::<_, ()>(
          &object,
          "org.freedesktop.DBus.Properties",
          "Set",
          &(
            "org.a11y.atspi.Value",
            "CurrentValue",
            zbus::zvariant::Value::from(value),
          ),
        )
      };

      match request {
        AccessRequest::Activate
        | AccessRequest::Toggle
        | AccessRequest::Expand
        | AccessRequest::Collapse => reader
          .call::<_, bool>(&object, "org.a11y.atspi.Action", "DoAction", &(0i32,))
          .map(drop),
        AccessRequest::Focus => reader
          .call::<_, bool>(&object, "org.a11y.atspi.Component", "GrabFocus", &())
          .map(drop),
        AccessRequest::SetValue(value) => set(value),
        AccessRequest::Increment | AccessRequest::Decrement => {
          let now = reader.value(&object)?;
          let step =
            f64::try_from(reader.property(&object, "org.a11y.atspi.Value", "MinimumIncrement")?)
              .map_err(|error| error.to_string())?;
          let step = if step > 0. { step } else { 1. };

          set(if request == AccessRequest::Increment {
            now + step
          } else {
            now - step
          })
        }
      }
    })
  }

  /// The key binding of `object`'s first action (its keyboard shortcut), empty where it has
  /// none.
  ///
  /// # Errors
  ///
  /// Where it has no actions.
  pub fn key_binding(&self, object: &Object) -> Result<String> {
    self.call(object, "org.a11y.atspi.Action", "GetKeyBinding", &(0i32,))
  }
}

/// AT-SPI's name for its role number `role`, as libatspi names it.
#[must_use]
pub const fn role_name(role: u32) -> &'static str {
  match role {
    1 => "accelerator label",
    2 => "alert",
    3 => "animation",
    4 => "arrow",
    5 => "calendar",
    6 => "canvas",
    7 => "check box",
    8 => "check menu item",
    9 => "color chooser",
    10 => "column header",
    11 => "combo box",
    12 => "date editor",
    16 => "dialog",
    18 => "drawing area",
    20 => "filler",
    23 => "frame",
    26 => "icon",
    27 => "image",
    29 => "label",
    31 => "list",
    32 => "list item",
    33 => "menu",
    34 => "menu bar",
    35 => "menu item",
    37 => "page tab",
    38 => "page tab list",
    39 => "panel",
    40 => "password text",
    41 => "popup menu",
    42 => "progress bar",
    43 => "push button",
    44 => "radio button",
    45 => "radio menu item",
    48 => "scroll bar",
    49 => "scroll pane",
    50 => "separator",
    51 => "slider",
    52 => "spin button",
    54 => "status bar",
    55 => "table",
    56 => "table cell",
    61 => "text",
    62 => "toggle button",
    63 => "tool bar",
    64 => "tool tip",
    65 => "tree",
    68 => "viewport",
    69 => "window",
    71 => "header",
    72 => "footer",
    73 => "paragraph",
    75 => "application",
    79 => "entry",
    81 => "caption",
    82 => "document frame",
    83 => "heading",
    85 => "section",
    87 => "form",
    88 => "link",
    90 => "table row",
    91 => "tree item",
    95 => "document web",
    98 => "list box",
    99 => "grouping",
    101 => "notification",
    103 => "level bar",
    109 => "article",
    110 => "landmark",
    111 => "log",
    116 => "static",
    129 => "push button menu",
    130 => "switch",
    _ => "unknown",
  }
}
