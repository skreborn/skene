//! A WebDriver session: the commands the harness sends a browser's driver over HTTP.

use std::time::Duration;

use serde_json::{Value, json};
use ureq::Agent;

/// The key a WebDriver element reference is held under.
const ELEMENT: &str = "element-6066-11e4-a52e-4f735466cecf";

/// An element of the page a session drives, by its WebDriver reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Element(String);

impl Element {
  /// The reference `value` holds, where it holds one.
  fn of(value: &Value) -> Option<Self> {
    value
      .get(ELEMENT)
      .and_then(Value::as_str)
      .map(|id| Self(id.to_owned()))
  }

  /// The element as a script's argument or an action's origin.
  pub(crate) fn to_value(&self) -> Value {
    json!({ ELEMENT: self.0 })
  }
}

/// A session on a browser's driver.
pub(crate) struct Session {
  /// The client every command goes through.
  agent: Agent,
  /// The session's URL on the driver, which every command's path extends.
  url: String,
  /// The browser's version, as the session reports it.
  version: String,
}

impl Session {
  /// Starts a session with `capabilities` on the driver serving `driver`, waiting up to
  /// `patience` for the driver to come up.
  pub(crate) fn start(
    driver: &str,
    capabilities: &Value,
    patience: Duration,
  ) -> Result<Self, String> {
    let agent: Agent = Agent::config_builder()
      .http_status_as_error(false)
      .build()
      .into();
    let deadline = std::time::Instant::now() + patience;

    while !send(&agent, "GET", &format!("{driver}/status"), None)
      .is_ok_and(|status| status.get("ready").and_then(Value::as_bool) == Some(true))
    {
      if std::time::Instant::now() > deadline {
        return Err(format!("the driver at {driver} never came up"));
      }

      std::thread::sleep(Duration::from_millis(50));
    }

    let created = send(
      &agent,
      "POST",
      &format!("{driver}/session"),
      Some(&json!({ "capabilities": { "alwaysMatch": capabilities } })),
    )?;
    let id = created
      .get("sessionId")
      .and_then(Value::as_str)
      .ok_or_else(|| format!("the driver created no session: {created}"))?;

    let version = created["capabilities"]["browserVersion"]
      .as_str()
      .unwrap_or_default()
      .to_owned();

    Ok(Self {
      agent,
      url: format!("{driver}/session/{id}"),
      version,
    })
  }

  /// The browser's version, as the session reports it.
  pub(crate) fn version(&self) -> &str {
    &self.version
  }

  /// Sends the command `method` `path` with `body`, returning its value.
  ///
  /// # Panics
  ///
  /// Panics where the driver answers with an error.
  fn command(&self, method: &str, path: &str, body: Option<&Value>) -> Value {
    send(&self.agent, method, &format!("{}{path}", self.url), body)
      .unwrap_or_else(|error| panic!("WebDriver {method} {path}: {error}"))
  }

  /// Ends the session, closing the browser.
  pub(crate) fn end(&self) {
    let _ = send(&self.agent, "DELETE", &self.url, None);
  }

  /// Navigates the current window to `url`.
  pub(crate) fn navigate(&self, url: &str) {
    self.command("POST", "/url", Some(&json!({ "url": url })));
  }

  /// The current window's handle.
  pub(crate) fn window(&self) -> String {
    self
      .command("GET", "/window", None)
      .as_str()
      .unwrap_or_default()
      .to_owned()
  }

  /// Opens a fresh window, returning its handle.
  pub(crate) fn new_window(&self) -> String {
    self.command("POST", "/window/new", Some(&json!({ "type": "window" })))["handle"]
      .as_str()
      .unwrap_or_default()
      .to_owned()
  }

  /// Makes the window `handle` the current one.
  pub(crate) fn switch_to(&self, handle: &str) {
    self.command("POST", "/window", Some(&json!({ "handle": handle })));
  }

  /// Closes the current window.
  pub(crate) fn close_window(&self) {
    self.command("DELETE", "/window", None);
  }

  /// Sizes the current window's outer frame to `width` by `height` CSS pixels.
  pub(crate) fn set_window_size(&self, width: f64, height: f64) {
    self.command(
      "POST",
      "/window/rect",
      Some(&json!({ "width": width, "height": height })),
    );
  }

  /// Every element `using` the strategy finds by `selector`, in tree order.
  pub(crate) fn elements(&self, using: &str, selector: &str) -> Vec<Element> {
    self
      .command(
        "POST",
        "/elements",
        Some(&json!({ "using": using, "value": selector })),
      )
      .as_array()
      .map(|found| found.iter().filter_map(Element::of).collect())
      .unwrap_or_default()
  }

  /// The element command `command` on `element`, its value.
  fn element(&self, element: &Element, command: &str) -> Value {
    self.command("GET", &format!("/element/{}/{command}", element.0), None)
  }

  /// The role the browser computes for `element`.
  pub(crate) fn computed_role(&self, element: &Element) -> String {
    self
      .element(element, "computedrole")
      .as_str()
      .unwrap_or_default()
      .to_owned()
  }

  /// The accessible name the browser computes for `element`.
  pub(crate) fn computed_label(&self, element: &Element) -> String {
    self
      .element(element, "computedlabel")
      .as_str()
      .unwrap_or_default()
      .to_owned()
  }

  /// Whether `element` is displayed, by WebDriver's displayedness.
  pub(crate) fn is_displayed(&self, element: &Element) -> bool {
    self.element(element, "displayed").as_bool() == Some(true)
  }

  /// Whether `element` is selected: a checkbox or a radio checked, an option chosen.
  pub(crate) fn is_selected(&self, element: &Element) -> bool {
    self.element(element, "selected").as_bool() == Some(true)
  }

  /// Whether `element` is enabled: a form control not disabled.
  pub(crate) fn is_enabled(&self, element: &Element) -> bool {
    self.element(element, "enabled").as_bool() == Some(true)
  }

  /// The DOM property `name` of `element`.
  pub(crate) fn property(&self, element: &Element, name: &str) -> Value {
    self.element(element, &format!("property/{name}"))
  }

  /// Runs `script` as a function's body with `arguments`, returning what it returns.
  pub(crate) fn execute(&self, script: &str, arguments: &[Value]) -> Value {
    self.command(
      "POST",
      "/execute/sync",
      Some(&json!({ "script": script, "args": arguments })),
    )
  }

  /// Runs `script` as a function's body with `arguments` and a callback last, returning
  /// what it passes the callback.
  pub(crate) fn execute_async(&self, script: &str, arguments: &[Value]) -> Value {
    self.command(
      "POST",
      "/execute/async",
      Some(&json!({ "script": script, "args": arguments })),
    )
  }

  /// The element `script` returns, run as [`execute`](Self::execute) does.
  pub(crate) fn execute_element(&self, script: &str, arguments: &[Value]) -> Option<Element> {
    Element::of(&self.execute(script, arguments))
  }

  /// The current window's viewport as the browser draws it, as a PNG.
  pub(crate) fn screenshot(&self) -> Vec<u8> {
    use base64::Engine as _;

    let encoded = self.command("GET", "/screenshot", None);

    base64::engine::general_purpose::STANDARD
      .decode(encoded.as_str().unwrap_or_default())
      .unwrap_or_else(|error| panic!("WebDriver GET /screenshot: {error}"))
  }

  /// Performs the input source `sources`' actions, as a user's input.
  pub(crate) fn perform(&self, sources: &[Value]) {
    self.command("POST", "/actions", Some(&json!({ "actions": sources })));
  }
}

/// Sends `method` `url` with `body`, returning the reply's value, or the error the
/// driver reports.
fn send(agent: &Agent, method: &str, url: &str, body: Option<&Value>) -> Result<Value, String> {
  let response = match (method, body) {
    ("GET", _) => agent.get(url).call(),
    ("DELETE", _) => agent.delete(url).call(),
    (_, body) => agent
      .post(url)
      .header("Content-Type", "application/json; charset=utf-8")
      .send(body.map_or_else(|| "{}".to_owned(), Value::to_string)),
  }
  .map_err(|error| error.to_string())?;
  let text = response
    .into_body()
    .with_config()
    .limit(u64::MAX)
    .read_to_string()
    .map_err(|error| error.to_string())?;
  let reply: Value = serde_json::from_str(&text).map_err(|error| format!("{error}: {text}"))?;
  let value = reply.get("value").cloned().unwrap_or(Value::Null);

  match value.get("error").and_then(Value::as_str) {
    Some(error) => Err(format!(
      "{error}: {}",
      value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
    )),
    None => Ok(value),
  }
}
