//! Drag and drop on X11, XDND version 5, on both sides:
//!
//! - **a drop target**: the window announces `XdndAware`; a drag's `XdndPosition` is answered
//!   with an `XdndStatus` (whether a drop there would be taken, and how: [`Shell::drag_over`]),
//!   and a drop fetches what it takes from the `XdndSelection` before it says `XdndFinished`;
//! - **a drag source**: a drag from the window takes the pointer (the drag's cursor saying what
//!   the target under it answered), finds the `XdndAware` window under the pointer (its own
//!   included), offers it UTF-8 text, answers the `XdndSelection` with the text, and tells the
//!   shell how the drag ended ([`Shell::drag_ended`]).

use std::{
  error::Error,
  time::{Duration, Instant},
};

use x11rb::{
  protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, GrabMode, PropMode,
    SelectionNotifyEvent, SelectionRequestEvent, Window,
  },
  wrapper::ConnectionExt as _,
  xcb_ffi::XCBConnection,
};

use crate::{
  shell::{DropAction, Offered, Shell, Wanted},
  trace,
};

/// The XDND version spoken.
const VERSION: u32 = 5;

/// How long a drop waits for its target's `XdndFinished` before it is taken as refused.
const FINISH_PATIENCE: Duration = Duration::from_secs(2);

x11rb::atom_manager! {
  /// The atoms XDND names.
  Atoms: AtomsCookie {
    XdndAware,
    XdndEnter,
    XdndPosition,
    XdndStatus,
    XdndLeave,
    XdndDrop,
    XdndFinished,
    XdndSelection,
    XdndTypeList,
    XdndActionCopy,
    XdndActionMove,
    XdndActionList,
    UTF8_STRING,
    STRING,
    TEXT_PLAIN: b"text/plain;charset=utf-8",
    TEXT_PLAIN_ASCII: b"text/plain",
    URI_LIST: b"text/uri-list",
    SKENE_DND,
  }
}

/// A drag coming into the window.
struct Incoming {
  /// The source's window.
  source: Window,
  /// The types it offers.
  types: Vec<u32>,
  /// Where it was last, in surface pixels.
  at: (f32, f32),
  /// The drop's time, once dropped.
  dropped: Option<u32>,
}

/// A drag going out of the window.
struct Outgoing {
  /// The text it carries.
  text: String,
  /// Whether it may move the text.
  moves: bool,
  /// The `XdndAware` window under the pointer, and the version it speaks.
  target: Option<(Window, u32)>,
  /// What the target last answered: the action a drop would do, `None` where it refused.
  status: Option<DropAction>,
  /// When it was dropped, while its target's `XdndFinished` is awaited.
  dropped: Option<Instant>,
}

/// The window's drag and drop.
pub struct Xdnd {
  atoms: Atoms,
  incoming: Option<Incoming>,
  outgoing: Option<Outgoing>,
}

/// What the backend does after a drag's event: the cursor to show, where the drag's status
/// changed it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragCursor {
  /// A drop here would copy.
  Copy,
  /// A drop here would move.
  Move,
  /// Nothing here takes the drop.
  None,
}

impl DragCursor {
  /// The cursor's name in the freedesktop cursor theme.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Copy => "copy",
      Self::Move => "move",
      Self::None => "no-drop",
    }
  }
}

impl Xdnd {
  /// Interns XDND's atoms and announces `window` as a drop target.
  ///
  /// # Errors
  ///
  /// Where the connection fails.
  pub fn new(conn: &XCBConnection, window: Window) -> Result<Self, Box<dyn Error>> {
    let atoms = Atoms::new(conn)?.reply()?;

    conn.change_property32(
      PropMode::REPLACE,
      window,
      atoms.XdndAware,
      AtomEnum::ATOM,
      &[VERSION],
    )?;

    Ok(Self {
      atoms,
      incoming: None,
      outgoing: None,
    })
  }

  /// Whether a drag from the window is in progress.
  pub const fn is_dragging(&self) -> bool {
    self.outgoing.is_some()
  }

  /// What a drag offering `types` offers, in `atoms`' names.
  fn offered(atoms: &Atoms, types: &[u32]) -> Offered {
    Offered {
      text: types.iter().any(|kind| {
        [
          atoms.UTF8_STRING,
          atoms.TEXT_PLAIN,
          atoms.TEXT_PLAIN_ASCII,
          atoms.STRING,
        ]
        .contains(kind)
      }),
      uris: types.contains(&atoms.URI_LIST),
    }
  }

  /// The action an XDND action atom names, in `atoms`' names.
  fn action_of(atoms: &Atoms, atom: u32) -> Option<DropAction> {
    if atom == atoms.XdndActionMove {
      Some(DropAction::Move)
    } else if atom == atoms.XdndActionCopy {
      Some(DropAction::Copy)
    } else {
      None
    }
  }

  /// The atom naming `action`.
  const fn atom_of(&self, action: DropAction) -> u32 {
    match action {
      DropAction::Copy => self.atoms.XdndActionCopy,
      DropAction::Move => self.atoms.XdndActionMove,
    }
  }

  /// Sends XDND message `kind` with `data` to `target`.
  fn send(
    &self,
    conn: &XCBConnection,
    target: Window,
    kind: u32,
    data: [u32; 5],
  ) -> Result<(), Box<dyn Error>> {
    conn.send_event(
      false,
      target,
      EventMask::NO_EVENT,
      ClientMessageEvent::new(32, target, kind, data),
    )?;

    Ok(())
  }

  /// Handles an XDND client message for `window`, at the window's `origin` on the screen
  /// (device pixels) and `scale`: whether it was one, and the drag's cursor where it changed.
  ///
  /// # Errors
  ///
  /// Where the connection fails.
  pub fn client_message(
    &mut self,
    conn: &XCBConnection,
    shell: &mut Shell,
    window: Window,
    event: &ClientMessageEvent,
    origin: (i32, i32),
  ) -> Result<(bool, Option<DragCursor>), Box<dyn Error>> {
    let atoms = self.atoms;
    let data = event.data.as_data32();
    let kind = event.type_;

    if kind == atoms.XdndEnter {
      let source = data[0];
      let types: Vec<u32> = if data[1] & 1 == 0 {
        data[2..5]
          .iter()
          .copied()
          .filter(|atom| *atom != 0)
          .collect()
      } else {
        conn
          .get_property(false, source, atoms.XdndTypeList, AtomEnum::ATOM, 0, 64)?
          .reply()?
          .value32()
          .map(Iterator::collect)
          .unwrap_or_default()
      };

      trace::log(|| format!("[dnd] enter from 0x{source:x}, {} types", types.len()));
      self.incoming = Some(Incoming {
        source,
        types,
        at: (0., 0.),
        dropped: None,
      });

      return Ok((true, None));
    }

    if kind == atoms.XdndPosition {
      let Some(incoming) = &mut self.incoming else {
        return Ok((true, None));
      };
      let scale = shell.scale as f32;
      let (x, y) = ((data[2] >> 16) as i32, (data[2] & 0xffff) as i32);
      let at = ((x - origin.0) as f32 / scale, (y - origin.1) as f32 / scale);

      incoming.at = at;

      let source = incoming.source;
      let offered = Self::offered(&atoms, &incoming.types);
      let action = shell.drag_over(at.0, at.1, offered);
      let (flags, answer) = match action {
        Some(action) => (0b11, self.atom_of(action)),
        None => (0b10, x11rb::NONE),
      };

      self.send(
        conn,
        source,
        atoms.XdndStatus,
        [window, flags, 0, 0, answer],
      )?;

      return Ok((true, None));
    }

    if kind == atoms.XdndLeave {
      trace::log(|| "[dnd] leave".to_owned());
      self.incoming = None;
      shell.drag_left();

      return Ok((true, None));
    }

    if kind == atoms.XdndDrop {
      let Some(incoming) = &mut self.incoming else {
        return Ok((true, None));
      };
      let time = data[2];
      let source = incoming.source;
      let at = incoming.at;
      let offered = Self::offered(&atoms, &incoming.types);
      let wanted = shell.drop_wants(at.0, at.1, offered);
      let target = match wanted {
        Some(Wanted::Uris) => Some(atoms.URI_LIST),
        Some(Wanted::Text) => [
          atoms.UTF8_STRING,
          atoms.TEXT_PLAIN,
          atoms.TEXT_PLAIN_ASCII,
          atoms.STRING,
        ]
        .into_iter()
        .find(|kind| incoming.types.contains(kind)),
        None => None,
      };

      trace::log(|| format!("[dnd] drop at {at:?}: wants {wanted:?}"));

      match target {
        Some(target) => {
          incoming.dropped = Some(time);
          conn.convert_selection(window, atoms.XdndSelection, target, atoms.SKENE_DND, time)?;
        }
        None => {
          self.incoming = None;
          shell.drag_left();
          self.send(conn, source, atoms.XdndFinished, [window, 0, 0, 0, 0])?;
        }
      }

      return Ok((true, None));
    }

    if kind == atoms.XdndStatus {
      let Some(outgoing) = &mut self.outgoing else {
        return Ok((true, None));
      };
      let accepted = data[1] & 1 != 0;
      let action = accepted
        .then(|| Self::action_of(&atoms, data[4]).unwrap_or(DropAction::Copy))
        .filter(|action| *action == DropAction::Copy || outgoing.moves);

      outgoing.status = action;

      let cursor = match action {
        Some(DropAction::Copy) => DragCursor::Copy,
        Some(DropAction::Move) => DragCursor::Move,
        None => DragCursor::None,
      };

      return Ok((true, Some(cursor)));
    }

    if kind == atoms.XdndFinished {
      let Some(outgoing) = self.outgoing.take() else {
        return Ok((true, None));
      };
      let accepted = data[1] & 1 != 0;
      let action = accepted
        .then(|| Self::action_of(&atoms, data[2]).or(outgoing.status))
        .flatten();

      trace::log(|| format!("[dnd] finished: {action:?}"));
      shell.drag_ended(action);

      return Ok((true, None));
    }

    Ok((false, None))
  }

  /// What arrived for a drop's conversion of the `XdndSelection`: the drop is done with it, and
  /// its source told; whether it was the drop's.
  ///
  /// # Errors
  ///
  /// Where the connection fails.
  pub fn selection_notify(
    &mut self,
    conn: &XCBConnection,
    shell: &mut Shell,
    window: Window,
    event: &SelectionNotifyEvent,
  ) -> Result<bool, Box<dyn Error>> {
    if event.selection != self.atoms.XdndSelection {
      return Ok(false);
    }

    let Some(incoming) = self.incoming.take() else {
      return Ok(true);
    };
    let bytes = if event.property == x11rb::NONE {
      Vec::new()
    } else {
      conn
        .get_property(true, window, event.property, AtomEnum::ANY, 0, u32::MAX / 4)?
        .reply()?
        .value
    };
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let dropped = if event.target == self.atoms.URI_LIST {
      skene_foundation::seam::report::Dropped {
        text: None,
        uris: text
          .lines()
          .map(str::trim)
          .filter(|line| !line.is_empty() && !line.starts_with('#'))
          .map(std::rc::Rc::from)
          .collect(),
      }
    } else {
      skene_foundation::seam::report::Dropped {
        text: Some(std::rc::Rc::from(text.as_str())),
        uris: Vec::new(),
      }
    };
    let action = if bytes.is_empty() && event.property == x11rb::NONE {
      shell.drag_left();

      None
    } else {
      shell.drop_data(incoming.at.0, incoming.at.1, dropped)
    };
    let (flags, answer) = match action {
      Some(action) => (1, self.atom_of(action)),
      None => (0, x11rb::NONE),
    };

    self.send(
      conn,
      incoming.source,
      self.atoms.XdndFinished,
      [window, flags, answer, 0, 0],
    )?;

    Ok(true)
  }

  /// Answers a request for the `XdndSelection` with the dragged text; whether it was one.
  ///
  /// # Errors
  ///
  /// Where the connection fails.
  pub fn selection_request(
    &self,
    conn: &XCBConnection,
    request: &SelectionRequestEvent,
  ) -> Result<bool, Box<dyn Error>> {
    let atoms = self.atoms;

    if request.selection != atoms.XdndSelection {
      return Ok(false);
    }

    let property = if request.property == x11rb::NONE {
      request.target
    } else {
      request.property
    };
    let answered = match &self.outgoing {
      Some(outgoing)
        if [
          atoms.UTF8_STRING,
          atoms.TEXT_PLAIN,
          atoms.TEXT_PLAIN_ASCII,
          atoms.STRING,
        ]
        .contains(&request.target) =>
      {
        conn.change_property8(
          PropMode::REPLACE,
          request.requestor,
          property,
          request.target,
          outgoing.text.as_bytes(),
        )?;

        true
      }
      _ => false,
    };

    conn.send_event(
      false,
      request.requestor,
      EventMask::NO_EVENT,
      SelectionNotifyEvent {
        response_type: x11rb::protocol::xproto::SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: request.time,
        requestor: request.requestor,
        selection: request.selection,
        target: request.target,
        property: if answered { property } else { x11rb::NONE },
      },
    )?;

    Ok(true)
  }

  /// Starts a drag of `text` from `window` (one that may move it where `moves`) at `time`: the
  /// window owns the `XdndSelection` and takes the pointer, showing `cursor`.
  ///
  /// # Errors
  ///
  /// Where the connection fails.
  pub fn start(
    &mut self,
    conn: &XCBConnection,
    window: Window,
    text: String,
    moves: bool,
    time: u32,
    cursor: u32,
  ) -> Result<(), Box<dyn Error>> {
    conn.set_selection_owner(window, self.atoms.XdndSelection, time)?;

    let actions = if moves {
      vec![self.atoms.XdndActionCopy, self.atoms.XdndActionMove]
    } else {
      vec![self.atoms.XdndActionCopy]
    };

    conn.change_property32(
      PropMode::REPLACE,
      window,
      self.atoms.XdndActionList,
      AtomEnum::ATOM,
      &actions,
    )?;

    let grabbed = conn
      .grab_pointer(
        false,
        window,
        EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
        x11rb::NONE,
        cursor,
        x11rb::CURRENT_TIME,
      )?
      .reply()
      .map(|reply| reply.status);

    trace::log(|| {
      format!(
        "[dnd] start a drag of {} bytes, grab {grabbed:?}",
        text.len()
      )
    });
    self.outgoing = Some(Outgoing {
      text,
      moves,
      target: None,
      status: None,
      dropped: None,
    });

    Ok(())
  }

  /// The `XdndAware` window at `x`, `y` on the screen of `root`, and the version it speaks.
  fn aware_at(
    &self,
    conn: &XCBConnection,
    root: Window,
    x: i16,
    y: i16,
  ) -> Result<Option<(Window, u32)>, Box<dyn Error>> {
    let mut window = root;

    for _ in 0..16 {
      let child = conn
        .translate_coordinates(root, window, x, y)?
        .reply()?
        .child;

      if child == x11rb::NONE {
        return Ok(None);
      }

      window = child;

      let aware = conn
        .get_property(false, window, self.atoms.XdndAware, AtomEnum::ATOM, 0, 1)?
        .reply()?;

      if let Some(version) = aware.value32().and_then(|mut values| values.next()) {
        return Ok(Some((window, version)));
      }
    }

    Ok(None)
  }

  /// The pointer moved to `at` on the screen of `root` at `time` while the window drags, `shift`
  /// and `control` held or not: the target under it is told, the one it left told it left;
  /// the drag's cursor, where nothing takes it now.
  ///
  /// The drag suggests what GTK's does: a move with `Shift` alone held (where the text may
  /// move), else a copy; a target that would move it otherwise (the entry it came from)
  /// chooses from the window's `XdndActionList`.
  ///
  /// # Errors
  ///
  /// Where the connection fails.
  pub fn pointer_moved(
    &mut self,
    conn: &XCBConnection,
    window: Window,
    root: Window,
    at: (i16, i16),
    time: u32,
    (shift, control): (bool, bool),
  ) -> Result<Option<DragCursor>, Box<dyn Error>> {
    if self
      .outgoing
      .as_ref()
      .is_none_or(|out| out.dropped.is_some())
    {
      return Ok(None);
    }

    let atoms = self.atoms;
    let target = self.aware_at(conn, root, at.0, at.1)?;
    let Some(outgoing) = &mut self.outgoing else {
      return Ok(None);
    };
    let mut cursor = None;

    if outgoing.target.map(|(window, _)| window) != target.map(|(window, _)| window) {
      if let Some((old, _)) = outgoing.target.take() {
        conn.send_event(
          false,
          old,
          EventMask::NO_EVENT,
          ClientMessageEvent::new(32, old, atoms.XdndLeave, [window, 0, 0, 0, 0]),
        )?;
      }

      outgoing.status = None;
      cursor = Some(DragCursor::None);

      if let Some((new, version)) = target {
        let version = version.min(VERSION);

        conn.send_event(
          false,
          new,
          EventMask::NO_EVENT,
          ClientMessageEvent::new(
            32,
            new,
            atoms.XdndEnter,
            [
              window,
              version << 24,
              atoms.UTF8_STRING,
              atoms.TEXT_PLAIN,
              atoms.STRING,
            ],
          ),
        )?;
        outgoing.target = Some((new, version));
      }
    }

    if let Some((target, _)) = outgoing.target {
      let action = if outgoing.moves && shift && !control {
        atoms.XdndActionMove
      } else {
        atoms.XdndActionCopy
      };

      conn.send_event(
        false,
        target,
        EventMask::NO_EVENT,
        ClientMessageEvent::new(
          32,
          target,
          atoms.XdndPosition,
          [
            window,
            0,
            ((at.0 as u32) << 16) | (at.1 as u32 & 0xffff),
            time,
            action,
          ],
        ),
      )?;
    }

    Ok(cursor)
  }

  /// The button was released at `time`, ending the drag: a target that takes it is sent the
  /// drop (its `XdndFinished` ends the drag), else it ends refused. The pointer is given back.
  ///
  /// # Errors
  ///
  /// Where the connection fails.
  pub fn released(
    &mut self,
    conn: &XCBConnection,
    shell: &mut Shell,
    window: Window,
    time: u32,
  ) -> Result<(), Box<dyn Error>> {
    let atoms = self.atoms;
    let Some(outgoing) = &mut self.outgoing else {
      return Ok(());
    };

    conn.ungrab_pointer(x11rb::CURRENT_TIME)?;

    match (outgoing.target, outgoing.status) {
      (Some((target, _)), Some(_)) => {
        trace::log(|| format!("[dnd] drop on 0x{target:x}"));
        outgoing.dropped = Some(Instant::now());
        conn.send_event(
          false,
          target,
          EventMask::NO_EVENT,
          ClientMessageEvent::new(32, target, atoms.XdndDrop, [window, 0, time, 0, 0]),
        )?;
      }
      (target, _) => {
        if let Some((target, _)) = target {
          conn.send_event(
            false,
            target,
            EventMask::NO_EVENT,
            ClientMessageEvent::new(32, target, atoms.XdndLeave, [window, 0, 0, 0, 0]),
          )?;
        }

        trace::log(|| "[dnd] released over nothing that takes it".to_owned());
        self.outgoing = None;
        shell.drag_ended(None);
      }
    }

    Ok(())
  }

  /// Ends a drop whose target never said it finished, as refused.
  pub fn check_timeout(&mut self, shell: &mut Shell) {
    if self
      .outgoing
      .as_ref()
      .and_then(|outgoing| outgoing.dropped)
      .is_some_and(|dropped| dropped.elapsed() > FINISH_PATIENCE)
    {
      trace::log(|| "[dnd] the target never finished".to_owned());
      self.outgoing = None;
      shell.drag_ended(None);
    }
  }
}
