//! A stand-in for xdg-desktop-portal's Settings interface on a private session bus, for the
//! realization's environment checks: it serves `org.freedesktop.appearance`'s `color-scheme`
//! (0 no preference, 1 dark, 2 light), `contrast` (0 none, 1 more), `reduced-motion` (0 none,
//! 1 reduce) and `accent-color` (three fractions), and changes them a step at a time, emitting
//! `SettingChanged` as the portal does.
//!
//! `portal-stub <scheme> <contrast> [<key>=<value>@<step> ...]`, a color written `0.9,0.2,0.3`,
//! e.g. `portal-stub 2 0 color-scheme=1@1 accent-color=0.9,0.2,0.3@2`: each `SIGUSR1` it is sent
//! applies the next step's changes (steps 1, 2, ...). It exits after 120 seconds.

use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
  time::Duration,
};

use skene_shell_tools::Steps;
use zbus::{
  interface,
  object_server::SignalEmitter,
  zvariant::{OwnedValue, Structure, Value},
};

const NAMESPACE: &str = "org.freedesktop.appearance";

#[derive(Clone, Debug)]
enum Setting {
  Number(u32),
  Color(f64, f64, f64),
}

impl Setting {
  fn parse(text: &str) -> Option<Self> {
    let parts: Vec<&str> = text.split(',').collect();

    match parts.as_slice() {
      [number] => number.parse().ok().map(Self::Number),
      [red, green, blue] => Some(Self::Color(
        red.parse().ok()?,
        green.parse().ok()?,
        blue.parse().ok()?,
      )),
      _ => None,
    }
  }

  fn value(&self) -> OwnedValue {
    match self {
      Self::Number(number) => OwnedValue::from(*number),
      Self::Color(red, green, blue) => Value::Structure(Structure::from((*red, *green, *blue)))
        .try_into()
        .expect("a plain structure"),
    }
  }
}

struct Settings {
  values: Arc<Mutex<HashMap<String, Setting>>>,
}

#[interface(name = "org.freedesktop.portal.Settings")]
impl Settings {
  fn read_one(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
    let values = self.values.lock().unwrap();

    match (namespace, values.get(key)) {
      (NAMESPACE, Some(setting)) => Ok(setting.value()),
      _ => Err(zbus::fdo::Error::UnknownProperty(format!(
        "{namespace} {key}"
      ))),
    }
  }

  fn read(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
    let inner = self.read_one(namespace, key)?;

    Ok(
      Value::Value(Box::new(Value::from(inner)))
        .try_into()
        .expect("a plain value"),
    )
  }

  #[zbus(signal)]
  async fn setting_changed(
    emitter: &SignalEmitter<'_>,
    namespace: &str,
    key: &str,
    value: Value<'_>,
  ) -> zbus::Result<()>;

  #[zbus(property)]
  const fn version(&self) -> u32 {
    2
  }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Before the connection starts its threads.
  let steps = Steps::block();
  let started = std::time::Instant::now();
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let mut values = HashMap::new();

  values.insert(
    "color-scheme".to_owned(),
    Setting::Number(
      arguments
        .first()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0),
    ),
  );
  values.insert(
    "contrast".to_owned(),
    Setting::Number(
      arguments
        .get(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0),
    ),
  );
  values.insert("reduced-motion".to_owned(), Setting::Number(0));

  let values = Arc::new(Mutex::new(values));
  let connection = zbus::blocking::connection::Builder::session()?
    .name("org.freedesktop.portal.Desktop")?
    .serve_at(
      "/org/freedesktop/portal/desktop",
      Settings {
        values: Arc::clone(&values),
      },
    )?
    .build()?;

  println!("serving");

  let mut changes: Vec<(u32, String, Setting)> = arguments
    .iter()
    .skip(2)
    .filter_map(|change| {
      let (assignment, step) = change.split_once('@')?;
      let (key, value) = assignment.split_once('=')?;

      Some((step.parse().ok()?, key.to_owned(), Setting::parse(value)?))
    })
    .collect();

  changes.sort_by_key(|(step, _, _)| *step);

  let interface = connection
    .object_server()
    .interface::<_, Settings>("/org/freedesktop/portal/desktop")?;
  let lifetime = Duration::from_secs(120);
  let last = changes.last().map_or(0, |(step, _, _)| *step);

  for step in 1..=last {
    if !steps.next(lifetime.saturating_sub(started.elapsed())) {
      return Ok(());
    }

    for (_, key, setting) in changes.iter().filter(|(at, _, _)| *at == step) {
      values.lock().unwrap().insert(key.clone(), setting.clone());
      zbus::block_on(Settings::setting_changed(
        interface.signal_emitter(),
        NAMESPACE,
        key,
        Value::from(setting.value()),
      ))?;
      println!(
        "changed {key} to {setting:?} at step {step}, {} ms",
        started.elapsed().as_millis()
      );
    }
  }

  std::thread::sleep(lifetime.saturating_sub(started.elapsed()));

  Ok(())
}
