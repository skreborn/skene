//! The suite's driver over a WebDriver session: each scenario in a fresh window, what it
//! acts on found by the role and the name the browser computes, and every act a user's
//! trusted input from WebDriver's actions.

use std::time::Duration;

use serde_json::{Value, json};
use skene_conformance::{Driver, Reading};
use skene_foundation::{Key, KeyEvent, NamedKey, Role, Size};

use crate::{
  session::{Element, Session},
  site::Site,
};

/// The viewport a page opens at, in CSS pixels.
const VIEWPORT: Size<f32> = Size::new(1024., 768.);

/// How many times a resize sizes the window before its viewport must have the size
/// asked: a window's frame may change as it is resized.
const RESIZES: usize = 4;

/// How long a page may take to load and build its view.
const LOAD: Duration = Duration::from_secs(30);

/// Resolves once the page's view has run its effects and painted: two animation frames,
/// or a tenth of a second where the page is not drawn.
const SETTLE: &str = "const done = arguments[arguments.length - 1];
let settled = false;
const settle = () => { if (!settled) { settled = true; done(); } };
requestAnimationFrame(() => requestAnimationFrame(settle));
setTimeout(settle, 100);";

/// Resolves with whether the app built its page within `arguments[0]` milliseconds.
const READY: &str = "const [patience, done] = arguments;
const deadline = Date.now() + patience;
const poll = () => window.skeneConformance
  ? done(true)
  : Date.now() < deadline ? setTimeout(poll, 20) : done(false);
poll();";

/// Returns what the page publishes as the probe `arguments[0]`, or `null`.
const PROBE: &str = "return window.skeneConformance?.probe(arguments[0]) ?? null;";

/// Resolves with whether the pinned font loaded within `arguments[0]` milliseconds:
/// its stylesheet applied and its face ready to draw.
const FONTS: &str = "const [patience, done] = arguments;
const deadline = Date.now() + patience;
const applied = () => [...document.styleSheets]
  .some((sheet) => sheet.href && sheet.href.endsWith('/fonts/pinned.css'));
const poll = () => applied()
  ? document.fonts.load('16px \"Pinned Inter\"')
    .then(() => document.fonts.ready)
    .then(() => done(document.fonts.check('16px \"Pinned Inter\"')), () => done(false))
  : Date.now() < deadline ? setTimeout(poll, 20) : done(false);
poll();";

/// Returns the element holding focus, or `null` where the document's body holds it.
const FOCUSED: &str = "const focused = document.activeElement;
return focused && focused !== document.body && focused !== document.documentElement
  ? focused : null;";

/// A browser the suite drives through a WebDriver session, the suite's pages served by
/// the site.
pub(crate) struct Browser<'a> {
  /// The session on the browser.
  session: &'a Session,
  /// Where the pages are served.
  site: &'a Site,
}

impl<'a> Browser<'a> {
  /// The browser `session` drives, opening the pages `site` serves.
  pub(crate) const fn new(session: &'a Session, site: &'a Site) -> Self {
    Self { session, site }
  }

  /// Waits until the page has reacted to what was done to it.
  fn settle(&self) {
    self.session.execute_async(SETTLE, &[]);
  }

  /// Waits until the page draws in the pinned font.
  ///
  /// # Panics
  ///
  /// Panics where the font never loads.
  pub(crate) fn await_fonts(&self) {
    let patience = u64::try_from(LOAD.as_millis()).unwrap_or(u64::MAX);

    assert_eq!(
      self.session.execute_async(FONTS, &[json!(patience)]),
      json!(true),
      "the page draws in the pinned font"
    );

    self.settle();
  }

  /// The viewport as the browser draws it, as a PNG.
  pub(crate) fn screenshot(&self) -> Vec<u8> {
    self.session.screenshot()
  }

  /// The window's size and its viewport's, in CSS pixels.
  fn viewport(&self) -> Viewport {
    let sizes = self.session.execute(
      "return [outerWidth, outerHeight, innerWidth, innerHeight];",
      &[],
    );
    let [outer_width, outer_height, inner_width, inner_height] =
      [0, 1, 2, 3].map(|index| sizes[index].as_f64().unwrap_or_default());

    Viewport {
      outer: [outer_width, outer_height],
      inner: [inner_width, inner_height],
    }
  }

  /// Performs `sources`' actions, then waits until the page has reacted to them.
  fn perform(&self, sources: &[Value]) {
    self.session.perform(sources);
    self.settle();
  }
}

impl Driver for Browser<'_> {
  type Node = Element;
  type Scalar = f32;

  fn open(&mut self, page: &str) {
    let previous = self.session.window();
    let fresh = self.session.new_window();

    self.session.close_window();
    self.session.switch_to(&fresh);

    assert_ne!(previous, fresh, "a fresh window opened");

    self.resize(VIEWPORT);
    self.session.navigate(&self.site.page(page));

    let patience = u64::try_from(LOAD.as_millis()).unwrap_or(u64::MAX);

    assert_eq!(
      self.session.execute_async(READY, &[json!(patience)]),
      json!(true),
      "the page {page:?} built its view"
    );

    self.settle();
  }

  fn probe(&mut self, name: &str) -> String {
    match self.session.execute(PROBE, &[json!(name)]) {
      Value::String(value) => value,
      _ => panic!("the page publishes no probe {name:?}"),
    }
  }

  fn find(&mut self, role: Role, name: &str) -> Option<Element> {
    let expected =
      computed_role(role).unwrap_or_else(|| panic!("no computed role is known for {role:?}"));
    let name = collapsed(name);

    self
      .session
      .elements("css selector", "body *")
      .into_iter()
      .find(|element| {
        self.session.computed_role(element) == expected
          && collapsed(&self.session.computed_label(element)) == name
      })
  }

  fn find_text(&mut self, text: &str) -> Option<Element> {
    let selector = format!(
      "//body//*[text()[normalize-space(.) = {}]]",
      xpath_literal(&collapsed(text))
    );

    self
      .session
      .elements("xpath", &selector)
      .into_iter()
      .find(|element| self.session.is_displayed(element))
  }

  fn read(&mut self, node: &Element) -> Reading {
    let entry = matches!(
      self.session.computed_role(node).as_str(),
      "textbox" | "searchbox"
    );

    Reading {
      value: entry
        .then(|| {
          self
            .session
            .property(node, "value")
            .as_str()
            .map(str::to_owned)
        })
        .flatten(),
      checked: self.session.is_selected(node),
      disabled: !self.session.is_enabled(node),
    }
  }

  fn hover(&mut self, node: &Element) {
    self.perform(&[pointer(&[json!({
      "type": "pointerMove",
      "origin": node.to_value(),
      "x": 0,
      "y": 0,
    })])]);
  }

  fn click(&mut self, node: &Element) {
    self.perform(&[pointer(&[
      json!({ "type": "pointerMove", "origin": node.to_value(), "x": 0, "y": 0 }),
      json!({ "type": "pointerDown", "button": 0 }),
      json!({ "type": "pointerUp", "button": 0 }),
    ])]);
  }

  fn press(&mut self, key: &KeyEvent) {
    let value =
      key_value(&key.key).unwrap_or_else(|| panic!("WebDriver has no key value for {:?}", key.key));
    let held: Vec<char> = [
      (key.modifiers.shift, '\u{E008}'),
      (key.modifiers.ctrl, '\u{E009}'),
      (key.modifiers.alt, '\u{E00A}'),
      (key.modifiers.meta, '\u{E03D}'),
    ]
    .into_iter()
    .filter_map(|(held, modifier)| held.then_some(modifier))
    .collect();
    let mut actions: Vec<Value> = held
      .iter()
      .map(|modifier| json!({ "type": "keyDown", "value": modifier.to_string() }))
      .collect();

    actions.push(json!({ "type": "keyDown", "value": value }));
    actions.push(json!({ "type": "keyUp", "value": value }));
    actions.extend(
      held
        .iter()
        .rev()
        .map(|modifier| json!({ "type": "keyUp", "value": modifier.to_string() })),
    );

    self.perform(&[keyboard(&actions)]);
  }

  fn type_text(&mut self, node: &Element, text: &str) {
    self.click(node);

    let actions: Vec<Value> = text
      .chars()
      .flat_map(|character| {
        [
          json!({ "type": "keyDown", "value": character.to_string() }),
          json!({ "type": "keyUp", "value": character.to_string() }),
        ]
      })
      .collect();

    self.perform(&[keyboard(&actions)]);
  }

  fn focus(&mut self, node: &Element) {
    self
      .session
      .execute("arguments[0].focus();", &[node.to_value()]);
    self.settle();
  }

  fn resize(&mut self, size: Size<f32>) {
    let wanted = [f64::from(size.width), f64::from(size.height)];
    let mut viewport = self.viewport();

    for _ in 0..RESIZES {
      if viewport.holds(wanted) {
        return;
      }

      self.session.set_window_size(
        wanted[0] + viewport.outer[0] - viewport.inner[0],
        wanted[1] + viewport.outer[1] - viewport.inner[1],
      );
      self.settle();

      viewport = self.viewport();
    }

    assert!(
      viewport.holds(wanted),
      "the browser sizes its viewport to {size:?}, not {:?}",
      viewport.inner
    );
  }

  fn advance(&mut self, duration: Duration) {
    std::thread::sleep(duration);
    self.settle();
  }

  fn focused(&mut self) -> Option<Element> {
    self.session.execute_element(FOCUSED, &[])
  }
}

/// A window's size and its viewport's, each a width and a height in CSS pixels.
struct Viewport {
  outer: [f64; 2],
  inner: [f64; 2],
}

impl Viewport {
  /// Whether the viewport is `size`, to within a rounding of the device's pixels.
  fn holds(&self, size: [f64; 2]) -> bool {
    self
      .inner
      .iter()
      .zip(size)
      .all(|(held, wanted)| (held - wanted).abs() < 0.5)
  }
}

/// The mouse's input source performing `actions`.
fn pointer(actions: &[Value]) -> Value {
  json!({
    "type": "pointer",
    "id": "mouse",
    "parameters": { "pointerType": "mouse" },
    "actions": actions,
  })
}

/// The keyboard's input source performing `actions`.
fn keyboard(actions: &[Value]) -> Value {
  json!({ "type": "key", "id": "keyboard", "actions": actions })
}

/// The role a browser computes for a node a scene reports as `role`, as WebDriver's
/// computed role spells it; `None` for a role the harness knows no spelling of.
const fn computed_role(role: Role) -> Option<&'static str> {
  Some(match role {
    Role::Alert => "alert",
    Role::AlertDialog => "alertdialog",
    Role::Button => "button",
    Role::CheckBox => "checkbox",
    Role::ComboBox => "combobox",
    Role::Dialog => "dialog",
    Role::Group => "group",
    Role::Heading => "heading",
    Role::Link => "link",
    Role::ListBox => "listbox",
    Role::ListBoxOption => "option",
    Role::Menu => "menu",
    Role::MenuItem => "menuitem",
    Role::RadioButton => "radio",
    Role::RadioGroup => "radiogroup",
    Role::SearchInput => "searchbox",
    Role::Slider => "slider",
    Role::Status => "status",
    Role::Switch => "switch",
    Role::Tab => "tab",
    Role::TabList => "tablist",
    Role::TabPanel => "tabpanel",
    Role::TextInput | Role::MultilineTextInput => "textbox",
    Role::Tooltip => "tooltip",
    _ => return None,
  })
}

/// The WebDriver key value pressing `key` sends: its own text for a character, a
/// code point of WebDriver's for a named key; `None` for a key it has none for.
fn key_value(key: &Key) -> Option<String> {
  let named = match key {
    Key::Character(text) => return Some(text.clone()),
    Key::Named(named) => named,
  };
  let value = match named {
    NamedKey::Backspace => '\u{E003}',
    NamedKey::Tab => '\u{E004}',
    NamedKey::Enter => '\u{E007}',
    NamedKey::Shift => '\u{E008}',
    NamedKey::Control => '\u{E009}',
    NamedKey::Alt => '\u{E00A}',
    NamedKey::Escape => '\u{E00C}',
    NamedKey::PageUp => '\u{E00E}',
    NamedKey::PageDown => '\u{E00F}',
    NamedKey::End => '\u{E010}',
    NamedKey::Home => '\u{E011}',
    NamedKey::ArrowLeft => '\u{E012}',
    NamedKey::ArrowUp => '\u{E013}',
    NamedKey::ArrowRight => '\u{E014}',
    NamedKey::ArrowDown => '\u{E015}',
    NamedKey::Insert => '\u{E016}',
    NamedKey::Delete => '\u{E017}',
    NamedKey::F1 => '\u{E031}',
    NamedKey::F2 => '\u{E032}',
    NamedKey::F3 => '\u{E033}',
    NamedKey::F4 => '\u{E034}',
    NamedKey::F5 => '\u{E035}',
    NamedKey::F6 => '\u{E036}',
    NamedKey::F7 => '\u{E037}',
    NamedKey::F8 => '\u{E038}',
    NamedKey::F9 => '\u{E039}',
    NamedKey::F10 => '\u{E03A}',
    NamedKey::F11 => '\u{E03B}',
    NamedKey::F12 => '\u{E03C}',
    NamedKey::Meta => '\u{E03D}',
    _ => return None,
  };

  Some(value.to_string())
}

/// `text` with every run of whitespace collapsed to one space and none at either end.
fn collapsed(text: &str) -> String {
  text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` as an XPath string literal.
fn xpath_literal(text: &str) -> String {
  if !text.contains('\'') {
    return format!("'{text}'");
  }

  if !text.contains('"') {
    return format!("\"{text}\"");
  }

  let parts: Vec<String> = text.split('\'').map(|part| format!("'{part}'")).collect();

  format!("concat({})", parts.join(", \"'\", "))
}
