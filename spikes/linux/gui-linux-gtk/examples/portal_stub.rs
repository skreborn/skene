//! A stand-in for xdg-desktop-portal's Settings interface on a private session bus, for the
//! realization's environment checks: it serves `org.freedesktop.appearance` `color-scheme` (0
//! no preference, 1 dark, 2 light) and `contrast` (0 none, 1 more), and changes them on a
//! schedule, emitting `SettingChanged` as the portal does.
//!
//! `portal_stub <scheme> <contrast> [<key>=<value>@<milliseconds> ...]`, e.g.
//! `portal_stub 2 0 color-scheme=1@1500`. It exits after 60 seconds.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  time::Duration,
};

use skene_gui_linux::gtk::{gio, glib, prelude::*};

const PORTAL_XML: &str = r#"
<node>
  <interface name="org.freedesktop.portal.Settings">
    <method name="Read">
      <arg type="s" name="namespace" direction="in"/>
      <arg type="s" name="key" direction="in"/>
      <arg type="v" name="value" direction="out"/>
    </method>
    <method name="ReadOne">
      <arg type="s" name="namespace" direction="in"/>
      <arg type="s" name="key" direction="in"/>
      <arg type="v" name="value" direction="out"/>
    </method>
    <signal name="SettingChanged">
      <arg type="s" name="namespace"/>
      <arg type="s" name="key"/>
      <arg type="v" name="value"/>
    </signal>
    <property name="version" type="u" access="read"/>
  </interface>
</node>
"#;

fn main() {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let number = |index: usize| {
    arguments
      .get(index)
      .and_then(|value| value.parse::<u32>().ok())
      .unwrap_or(0)
  };
  let scheme = Rc::new(Cell::new(number(0)));
  let contrast = Rc::new(Cell::new(number(1)));
  let main = glib::MainLoop::new(None, false);
  let info = gio::DBusNodeInfo::for_xml(PORTAL_XML).expect("the interface parses");
  let interface = info
    .lookup_interface("org.freedesktop.portal.Settings")
    .expect("the interface is declared");
  let connection = Rc::new(RefCell::new(None::<gio::DBusConnection>));

  let _owner = gio::bus_own_name(
    gio::BusType::Session,
    "org.freedesktop.portal.Desktop",
    gio::BusNameOwnerFlags::NONE,
    {
      let (scheme, contrast, connection) = (scheme.clone(), contrast.clone(), connection.clone());

      move |bus, _| {
        let (scheme, contrast) = (scheme.clone(), contrast.clone());

        bus
          .register_object("/org/freedesktop/portal/desktop", &interface)
          .method_call(move |_, _, _, _, method, parameters, invocation| {
            let (namespace, key) = parameters.get::<(String, String)>().unwrap_or_default();
            let value = match (namespace.as_str(), key.as_str()) {
              ("org.freedesktop.appearance", "color-scheme") => Some(scheme.get()),
              ("org.freedesktop.appearance", "contrast") => Some(contrast.get()),
              _ => None,
            };

            match value {
              Some(value) => {
                let value = glib::Variant::from_variant(&value.to_variant());
                let value = if method == "Read" {
                  glib::Variant::from_variant(&value)
                } else {
                  value
                };

                invocation.return_value(Some(&glib::Variant::tuple_from_iter([value])));
              }
              None => invocation.return_dbus_error(
                "org.freedesktop.portal.Error.NotFound",
                "Requested setting not found",
              ),
            }
          })
          .property(|_, _, _, _, _| 2u32.to_variant())
          .build()
          .expect("the object registers");
        *connection.borrow_mut() = Some(bus);
      }
    },
    |_, name| eprintln!("STUB owns {name}"),
    |_, name| eprintln!("STUB lost {name}"),
  );

  for change in arguments.iter().skip(2) {
    let Some((setting, rest)) = change.split_once('=') else {
      continue;
    };
    let Some((value, after)) = rest.split_once('@') else {
      continue;
    };
    let (setting, value, after) = (
      setting.to_owned(),
      value.parse::<u32>().unwrap_or(0),
      after.parse::<u64>().unwrap_or(0),
    );
    let (scheme, contrast, connection) = (scheme.clone(), contrast.clone(), connection.clone());

    glib::timeout_add_local_once(Duration::from_millis(after), move || {
      match setting.as_str() {
        "color-scheme" => scheme.set(value),
        _ => contrast.set(value),
      }

      if let Some(bus) = connection.borrow().as_ref() {
        bus
          .emit_signal(
            None,
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "SettingChanged",
            Some(
              &(
                "org.freedesktop.appearance",
                setting.as_str(),
                value.to_variant(),
              )
                .to_variant(),
            ),
          )
          .expect("the signal is emitted");
        eprintln!("STUB SettingChanged {setting} {value}");
      }
    });
  }

  glib::timeout_add_local_once(Duration::from_secs(60), {
    let main = main.clone();

    move || main.quit()
  });
  main.run();
}
