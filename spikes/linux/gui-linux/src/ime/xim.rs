//! XIM through the `xim` crate (as Zed does): the server named by `XMODIFIERS=@im=...`, every
//! key forwarded to it, unhandled keys forwarded back, the preedit through callbacks
//! (on-the-spot, with status callbacks), or drawn by the server at the spot
//! (`SKENE_XIM_STYLE=position`); `callbacks-nothing` drops the status callbacks.

use std::{rc::Rc, time::Instant};

use x11rb::{protocol::xproto::KeyPressEvent, xcb_ffi::XCBConnection};
use xim::{
  AttributeName, Client as _, ClientCore as _, ClientError, ClientHandler, x11rb::X11rbClient,
};
use xim::{Feedback, ForwardEventFlag, InputStyle, Point, PreeditDrawStatus};

use super::{Ime, ImeEvent, RawKey};

type XClient = X11rbClient<Rc<XCBConnection>>;

/// The XIM conversation's state and what it produced since the last drain.
struct Handler {
  /// The client window.
  window: u32,
  /// The input style agreed.
  style: InputStyle,
  /// The input method id, once opened.
  im: Option<u16>,
  /// The input context id, once created.
  ic: Option<u16>,
  /// The events the server asks to be forwarded.
  forward_mask: u32,
  /// The composition shown.
  preedit: Vec<char>,
  /// What the conversation produced since the last drain.
  events: Vec<ImeEvent>,
  /// When each forwarded key was sent, by (event type, keycode), for latency; the last one.
  sent: std::collections::HashMap<(u8, u8), Instant>,
  /// When the last forwarded key was sent.
  last: Option<Instant>,
  /// Where the caret is, sent as the spot.
  spot: Option<(i16, i16)>,
}

impl Handler {
  /// A commit or a preedit answers the last key sent.
  fn answered(&mut self, what: &str) {
    if let Some(sent) = self.last.take() {
      super::log_latency("xim", what, sent);
    }
  }

  fn spot(&self, client: &mut XClient) -> Result<(), ClientError> {
    let (Some(im), Some(ic), Some((x, y))) = (self.im, self.ic, self.spot) else {
      return Ok(());
    };
    let attributes = client
      .build_ic_attributes()
      .nested_list(AttributeName::PreeditAttributes, |list| {
        list.push(AttributeName::SpotLocation, Point { x, y });
      })
      .build();

    client.set_ic_values(im, ic, attributes)
  }
}

impl ClientHandler<XClient> for Handler {
  fn handle_connect(&mut self, client: &mut XClient) -> Result<(), ClientError> {
    self.events.push(ImeEvent::Note("connected".into()));
    client.open("en_US")
  }

  fn handle_open(&mut self, client: &mut XClient, im: u16) -> Result<(), ClientError> {
    self.im = Some(im);

    let attributes = client
      .build_ic_attributes()
      .push(AttributeName::InputStyle, self.style)
      .push(AttributeName::ClientWindow, self.window)
      .push(AttributeName::FocusWindow, self.window)
      .build();

    client.create_ic(im, attributes)
  }

  fn handle_create_ic(
    &mut self,
    client: &mut XClient,
    im: u16,
    ic: u16,
  ) -> Result<(), ClientError> {
    self.ic = Some(ic);
    self.events.push(ImeEvent::Ready(format!(
      "xim im {im} ic {ic} style {:?}",
      self.style
    )));
    client.set_focus(im, ic)?;
    self.spot(client)
  }

  fn handle_set_event_mask(
    &mut self,
    _client: &mut XClient,
    _im: u16,
    _ic: u16,
    forward_event_mask: u32,
    synchronous_event_mask: u32,
  ) -> Result<(), ClientError> {
    self.forward_mask = forward_event_mask;
    self.events.push(ImeEvent::Note(format!(
      "event mask forward 0x{forward_event_mask:x} sync 0x{synchronous_event_mask:x}"
    )));
    Ok(())
  }

  fn handle_commit(
    &mut self,
    _client: &mut XClient,
    _im: u16,
    _ic: u16,
    text: &str,
  ) -> Result<(), ClientError> {
    self.answered("commit");
    self.events.push(ImeEvent::Commit(text.into()));
    Ok(())
  }

  fn handle_forward_event(
    &mut self,
    _client: &mut XClient,
    _im: u16,
    _ic: u16,
    _flag: ForwardEventFlag,
    xev: KeyPressEvent,
  ) -> Result<(), ClientError> {
    if let Some(sent) = self.sent.remove(&(xev.response_type & 0x7f, xev.detail)) {
      super::log_latency("xim", "forward", sent);
    }

    self.events.push(ImeEvent::Note(format!(
      "forwarded back: type {} keycode {}",
      xev.response_type, xev.detail
    )));
    self.events.push(ImeEvent::Forward {
      keysym: 0,
      keycode: Some(u32::from(xev.detail)),
      state: u32::from(u16::from(xev.state)),
      release: xev.response_type & 0x7f == x11rb::protocol::xproto::KEY_RELEASE_EVENT,
    });
    Ok(())
  }

  fn handle_preedit_start(
    &mut self,
    client: &mut XClient,
    im: u16,
    ic: u16,
  ) -> Result<(), ClientError> {
    self.events.push(ImeEvent::Note("preedit start".into()));

    // The xim crate leaves XIM_PREEDIT_START unanswered; the protocol expects a reply with
    // the preedit's maximum length (-1: no limit).
    client.send_req(xim::Request::PreeditStartReply {
      input_method_id: im,
      input_context_id: ic,
      return_value: -1,
    })
  }

  fn handle_preedit_draw(
    &mut self,
    _client: &mut XClient,
    _im: u16,
    _ic: u16,
    caret: i32,
    chg_first: i32,
    chg_len: i32,
    status: PreeditDrawStatus,
    preedit_string: &str,
    _feedbacks: Vec<Feedback>,
  ) -> Result<(), ClientError> {
    self.answered("preedit");

    let first = (chg_first.max(0) as usize).min(self.preedit.len());
    let end = (first + chg_len.max(0) as usize).min(self.preedit.len());
    let inserted: Vec<char> = if status.contains(PreeditDrawStatus::NO_STRING) {
      Vec::new()
    } else {
      preedit_string.chars().collect()
    };

    self.preedit.splice(first..end, inserted);

    let text: String = self.preedit.iter().collect();

    self
      .events
      .push(ImeEvent::Preedit(text, u32::try_from(caret).ok()));
    Ok(())
  }

  fn handle_preedit_done(
    &mut self,
    _client: &mut XClient,
    _im: u16,
    _ic: u16,
  ) -> Result<(), ClientError> {
    self.preedit.clear();
    self.events.push(ImeEvent::Preedit(String::new(), None));
    Ok(())
  }
}

/// An XIM connection with one input context on the app's window.
pub struct Xim {
  client: XClient,
  handler: Handler,
}

impl Xim {
  /// Connects to the XIM server `XMODIFIERS` names, its input context on `window`.
  pub fn connect(
    conn: Rc<XCBConnection>,
    screen: usize,
    window: u32,
  ) -> Result<Self, Box<dyn std::error::Error>> {
    let client = XClient::init(conn, screen, None).map_err(|error| format!("{error}"))?;
    let style = match std::env::var("SKENE_XIM_STYLE").as_deref() {
      Ok("position") => InputStyle::PREEDIT_POSITION | InputStyle::STATUS_NOTHING,
      Ok("callbacks-nothing") => InputStyle::PREEDIT_CALLBACKS | InputStyle::STATUS_NOTHING,
      // ibus-x11 shows its candidate window only for a client with status callbacks, as
      // GTK 3's XIM module asks for.
      _ => InputStyle::PREEDIT_CALLBACKS | InputStyle::STATUS_CALLBACKS,
    };

    Ok(Self {
      client,
      handler: Handler {
        window,
        style,
        im: None,
        ic: None,
        forward_mask: 1,
        preedit: Vec::new(),
        events: Vec::new(),
        sent: std::collections::HashMap::new(),
        last: None,
        spot: None,
      },
    })
  }
}

impl Ime for Xim {
  fn name(&self) -> &'static str {
    "xim"
  }

  fn answers_keys(&self) -> bool {
    false
  }

  fn process_key(&mut self, _id: u64, key: &RawKey) -> bool {
    let (Some(im), Some(ic)) = (self.handler.im, self.handler.ic) else {
      return false;
    };
    // KeyPressMask 1, KeyReleaseMask 2.
    let wanted = if key.release { 2 } else { 1 };

    if self.handler.forward_mask & wanted == 0 {
      return false;
    }

    let now = Instant::now();

    self
      .handler
      .sent
      .insert((key.event.response_type & 0x7f, key.event.detail), now);
    self.handler.last = Some(now);

    match self
      .client
      .forward_event(im, ic, ForwardEventFlag::empty(), &key.event)
    {
      Ok(()) => true,
      Err(error) => {
        crate::trace::log(|| format!("[xim] forward failed: {error}"));
        false
      }
    }
  }

  fn focus_in(&mut self) {
    if let (Some(im), Some(ic)) = (self.handler.im, self.handler.ic) {
      let _ = self.client.set_focus(im, ic);
    }
  }

  fn focus_out(&mut self) {
    if let (Some(im), Some(ic)) = (self.handler.im, self.handler.ic) {
      let _ = self.client.unset_focus(im, ic);
    }
  }

  fn reset(&mut self) {
    // xim 0.5 has no XIM_RESET_IC request.
  }

  fn set_cursor(&mut self, _root: (i32, i32, i32, i32), window: (i32, i32, i32, i32)) {
    let spot = (window.0 as i16, (window.1 + window.3) as i16);

    if self.handler.spot != Some(spot) {
      self.handler.spot = Some(spot);

      if let Err(error) = self.handler.spot(&mut self.client) {
        crate::trace::log(|| format!("[xim] spot failed: {error}"));
      }
    }
  }

  fn set_surrounding(&mut self, _text: &str, _cursor: u32, _anchor: u32) {}

  fn filter_event(&mut self, event: &x11rb::protocol::Event) -> bool {
    match self.client.filter_event(event, &mut self.handler) {
      Ok(consumed) => consumed,
      Err(error) => {
        self
          .handler
          .events
          .push(ImeEvent::Note(format!("error: {error}")));
        true
      }
    }
  }

  fn take_events(&mut self) -> Vec<ImeEvent> {
    std::mem::take(&mut self.handler.events)
  }
}
