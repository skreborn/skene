//! A screen reader's view of the desktop: walks the AT-SPI tree over D-Bus (what Orca reads)
//! and prints each node as `role "name" [states] value=.. text=.. caret=.. rel=..`.
//!
//! `atspi-dump [app-name]` limits the walk to applications whose name contains the argument.

use skene_gui::platform::linux::gtk::{gio, glib};

const STATES: &[(u32, &str)] = &[
  (1, "active"),
  (4, "checked"),
  (7, "editable"),
  (8, "enabled"),
  (10, "expanded"),
  (11, "focusable"),
  (12, "focused"),
  (17, "multi-line"),
  (20, "pressed"),
  (22, "selectable"),
  (23, "selected"),
  (24, "sensitive"),
  (26, "single-line"),
  (32, "indeterminate"),
  (33, "required"),
  (36, "invalid-entry"),
  (41, "checkable"),
  (42, "has-popup"),
  (43, "read-only"),
];

const RELATIONS: &[(u32, &str)] = &[
  (1, "label-for"),
  (2, "labelled-by"),
  (3, "controller-for"),
  (4, "controlled-by"),
  (5, "member-of"),
  (17, "description-for"),
  (18, "described-by"),
];

struct Bus(gio::DBusConnection);

impl Bus {
  fn call(
    &self,
    dest: &str,
    path: &str,
    iface: &str,
    method: &str,
    args: Option<&glib::Variant>,
  ) -> Option<glib::Variant> {
    self
      .0
      .call_sync(
        Some(dest),
        path,
        iface,
        method,
        args,
        None,
        gio::DBusCallFlags::NONE,
        3000,
        gio::Cancellable::NONE,
      )
      .ok()
  }

  fn property(&self, dest: &str, path: &str, iface: &str, name: &str) -> Option<glib::Variant> {
    let args = (iface, name).into();
    let reply = self.call(
      dest,
      path,
      "org.freedesktop.DBus.Properties",
      "Get",
      Some(&args),
    )?;

    reply.child_value(0).as_variant()
  }

  fn string_property(&self, dest: &str, path: &str, name: &str) -> String {
    self
      .property(dest, path, "org.a11y.atspi.Accessible", name)
      .and_then(|v| v.str().map(str::to_owned))
      .unwrap_or_default()
  }

  fn children(&self, dest: &str, path: &str) -> Vec<(String, String)> {
    let Some(reply) = self.call(dest, path, "org.a11y.atspi.Accessible", "GetChildren", None)
    else {
      return Vec::new();
    };

    reply
      .child_value(0)
      .iter()
      .map(|child| {
        (
          child.child_value(0).str().unwrap_or_default().to_owned(),
          child.child_value(1).str().unwrap_or_default().to_owned(),
        )
      })
      .collect()
  }

  fn dump(&self, dest: &str, path: &str, depth: usize, out: &mut Vec<String>) {
    if depth > 40 {
      return;
    }

    let role = self
      .call(dest, path, "org.a11y.atspi.Accessible", "GetRoleName", None)
      .and_then(|r| r.child_value(0).str().map(str::to_owned))
      .unwrap_or_default();
    let name = self.string_property(dest, path, "Name");
    let description = self.string_property(dest, path, "Description");
    let mut states = Vec::new();

    if let Some(reply) = self.call(dest, path, "org.a11y.atspi.Accessible", "GetState", None) {
      let words: Vec<u32> = reply
        .child_value(0)
        .iter()
        .filter_map(|w| w.get::<u32>())
        .collect();

      for (bit, label) in STATES {
        let word = words.get((*bit / 32) as usize).copied().unwrap_or(0);

        if word & (1 << (bit % 32)) != 0 {
          states.push(*label);
        }
      }
    }

    let interfaces: Vec<String> = self
      .call(
        dest,
        path,
        "org.a11y.atspi.Accessible",
        "GetInterfaces",
        None,
      )
      .map(|r| {
        r.child_value(0)
          .iter()
          .filter_map(|s| s.str().map(str::to_owned))
          .collect()
      })
      .unwrap_or_default();
    let mut line = format!(
      "{}{role} {name:?} [{}]",
      "  ".repeat(depth),
      states.join(",")
    );

    if !description.is_empty() {
      line += &format!(" desc={description:?}");
    }

    if interfaces.iter().any(|i| i == "org.a11y.atspi.Value") {
      let get = |name| {
        self
          .property(dest, path, "org.a11y.atspi.Value", name)
          .and_then(|v| v.get::<f64>())
          .unwrap_or(f64::NAN)
      };

      line += &format!(
        " value={} min={} max={}",
        get("CurrentValue"),
        get("MinimumValue"),
        get("MaximumValue")
      );
    }

    if interfaces.iter().any(|i| i == "org.a11y.atspi.Text") {
      let count = self
        .property(dest, path, "org.a11y.atspi.Text", "CharacterCount")
        .and_then(|v| v.get::<i32>())
        .unwrap_or(-1);
      let get = |end: i32| {
        self
          .call(
            dest,
            path,
            "org.a11y.atspi.Text",
            "GetText",
            Some(&(0i32, end).into()),
          )
          .and_then(|r| r.child_value(0).str().map(str::to_owned))
          .unwrap_or_default()
      };
      let whole = get(-1);
      let counted = get(count.max(0));
      let text = if whole == counted {
        whole
      } else {
        format!("{whole:?} (0..-1) / {counted:?} (0..{count})")
      };
      let selection = self
        .call(
          dest,
          path,
          "org.a11y.atspi.Text",
          "GetSelection",
          Some(&(0i32,).into()),
        )
        .map(|r| {
          (
            r.child_value(0).get::<i32>().unwrap_or(-1),
            r.child_value(1).get::<i32>().unwrap_or(-1),
          )
        })
        .unwrap_or((-1, -1));

      line += &format!(" chars={count} selection={}..{}", selection.0, selection.1);
      let caret = self
        .property(dest, path, "org.a11y.atspi.Text", "CaretOffset")
        .and_then(|v| v.get::<i32>())
        .unwrap_or(-1);

      line += &format!(" text={text:?} caret={caret}");
    }

    if let Some(reply) = self.call(
      dest,
      path,
      "org.a11y.atspi.Accessible",
      "GetRelationSet",
      None,
    ) {
      for relation in reply.child_value(0).iter() {
        let kind = relation.child_value(0).get::<u32>().unwrap_or(0);
        let label = RELATIONS
          .iter()
          .find(|(k, _)| *k == kind)
          .map_or("other", |(_, l)| l);
        let targets: Vec<String> = relation
          .child_value(1)
          .iter()
          .map(|t| {
            let (d, p) = (
              t.child_value(0).str().unwrap_or_default().to_owned(),
              t.child_value(1).str().unwrap_or_default().to_owned(),
            );

            format!("{:?}", self.string_property(&d, &p, "Name"))
          })
          .collect();

        line += &format!(" {label}={}", targets.join("|"));
      }
    }

    out.push(line);

    for (d, p) in self.children(dest, path) {
      self.dump(&d, &p, depth + 1, out);
    }
  }
}

fn main() {
  let filter = std::env::args().nth(1).unwrap_or_default();
  let session =
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).expect("a session bus");
  let address = session
    .call_sync(
      Some("org.a11y.Bus"),
      "/org/a11y/bus",
      "org.a11y.Bus",
      "GetAddress",
      None,
      None,
      gio::DBusCallFlags::NONE,
      3000,
      gio::Cancellable::NONE,
    )
    .expect("the accessibility bus")
    .child_value(0)
    .str()
    .unwrap_or_default()
    .to_owned();
  let connection = gio::DBusConnection::for_address_sync(
    &address,
    gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
      | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
    None,
    gio::Cancellable::NONE,
  )
  .expect("connects to the accessibility bus");
  let bus = Bus(connection);
  let mut out = Vec::new();

  for (dest, path) in bus.children("org.a11y.atspi.Registry", "/org/a11y/atspi/accessible/root") {
    let name = bus.string_property(&dest, &path, "Name");

    if !name.contains(&filter) {
      continue;
    }

    bus.dump(&dest, &path, 0, &mut out);
  }

  for line in out {
    println!("{line}");
  }
}
