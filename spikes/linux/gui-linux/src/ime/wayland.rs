//! text-input-v3: the compositor relays an input method (IBus behind mutter, Fcitx5 behind
//! sway or KWin) to the focused entry. What arrives between two `done`s is applied together,
//! in the protocol's order (the composition removed, the text around the caret deleted, the
//! commit inserted, the new composition shown); the entry's state goes back with one `commit`
//! per change, and a state answering the input method's own change only once its `done`
//! carried our commit count, as GTK does.

use sctk::reexports::protocols::wp::text_input::zv3::client::{
  zwp_text_input_manager_v3::ZwpTextInputManagerV3,
  zwp_text_input_v3::{self, ChangeCause, ContentHint, ContentPurpose, ZwpTextInputV3},
};
use wayland_client::{Connection, QueueHandle, globals::GlobalList, protocol::wl_seat};

use crate::{entry::ImeState, shell::Shell, trace};

/// What a `done` brings: the composition with its caret, the commit, the deletion.
#[derive(Default)]
struct Pending {
  preedit: Option<(String, i32, i32)>,
  commit: Option<String>,
  delete: Option<(u32, u32)>,
}

/// The text-input-v3 client: what was last sent, and what arrives until `done`.
#[derive(Default)]
pub struct TextInput {
  /// The compositor's text-input manager.
  manager: Option<ZwpTextInputManagerV3>,
  /// The seat's text input.
  object: Option<ZwpTextInputV3>,
  /// Whether the compositor sent `enter` for the window's surface.
  entered: bool,
  /// Whether `enable` was sent with no `disable` since.
  enabled: bool,
  /// The `commit` requests sent: `done`'s serial echoes the count.
  commits: u32,
  /// The serial of the last `done`.
  done_serial: u32,
  /// What arrived since the last `done`.
  pending: Pending,
  /// The state last sent, the entry's rectangle in surface pixels.
  sent: Option<(ImeState, (i32, i32, i32, i32))>,
  /// Whether the entry's state must be sent again.
  stale: bool,
  /// Whether the input method made the last change.
  cause_input: bool,
}

/// User data of the text-input objects.
pub struct Raw;

impl TextInput {
  /// Binds the manager where the compositor offers one.
  pub fn bind<D>(globals: &GlobalList, qh: &QueueHandle<D>) -> Self
  where
    D: wayland_client::Dispatch<ZwpTextInputManagerV3, Raw> + 'static,
  {
    let manager = globals
      .bind::<ZwpTextInputManagerV3, _, _>(qh, 1..=1, Raw)
      .ok();

    trace::log(|| format!("[ime] text-input-v3 {}", manager.is_some()));

    Self {
      manager,
      stale: true,
      ..Self::default()
    }
  }

  /// Makes the seat's text input, once.
  pub fn new_seat<D>(&mut self, seat: &wl_seat::WlSeat, qh: &QueueHandle<D>)
  where
    D: wayland_client::Dispatch<ZwpTextInputV3, Raw> + 'static,
  {
    if let Some(manager) = &self.manager
      && self.object.is_none()
    {
      self.object = Some(manager.get_text_input(seat, qh, Raw));
    }
  }

  /// Brings the compositor's state in line with the focused entry: enabled while the window
  /// has text-input focus and an entry has focus, then its surrounding text, the change's
  /// cause, its content type and the caret's rectangle (`origin` the window's top left in the
  /// surface), one `commit` per change.
  pub fn sync(&mut self, shell: &Shell, origin: (f32, f32)) {
    if let Some(cause) = crate::entry::take_ime_dirty() {
      self.stale = true;
      self.cause_input |= cause;
    }

    if !self.stale || !self.entered {
      return;
    }

    let Some(object) = self.object.clone() else {
      return;
    };
    let state = shell.ime_state();

    self.stale = false;

    let cause_input = std::mem::take(&mut self.cause_input);
    let Some(state) = state else {
      self.disable();

      return;
    };
    let rect = (
      (state.rect.x + origin.0).round() as i32,
      (state.rect.y + origin.1).round() as i32,
      (state.rect.width.round() as i32).max(1),
      (state.rect.height.round() as i32).max(1),
    );

    if self.enabled && self.sent.as_ref() == Some(&(state.clone(), rect)) {
      return;
    }

    if self.enabled && cause_input && self.done_serial != self.commits {
      trace::log(|| {
        format!(
          "[ime] state held: done serial {} != commits {}",
          self.done_serial, self.commits
        )
      });

      return;
    }

    let enabling = !self.enabled;

    if enabling {
      object.enable();
      self.enabled = true;
    }

    object.set_surrounding_text(
      state.surrounding.clone(),
      i32::try_from(state.cursor).unwrap_or(0),
      i32::try_from(state.anchor).unwrap_or(0),
    );
    object.set_text_change_cause(if cause_input && !enabling {
      ChangeCause::InputMethod
    } else {
      ChangeCause::Other
    });

    let hint = match state.purpose {
      crate::ime::Purpose::Password => ContentHint::SensitiveData | ContentHint::HiddenText,
      _ if state.multiline => ContentHint::Multiline,
      _ => ContentHint::None,
    };
    let purpose = match state.purpose {
      crate::ime::Purpose::Normal => ContentPurpose::Normal,
      crate::ime::Purpose::Email => ContentPurpose::Email,
      crate::ime::Purpose::Number => ContentPurpose::Number,
      crate::ime::Purpose::Phone => ContentPurpose::Phone,
      crate::ime::Purpose::Url => ContentPurpose::Url,
      crate::ime::Purpose::Password => ContentPurpose::Password,
    };

    object.set_content_type(hint, purpose);
    object.set_cursor_rectangle(rect.0, rect.1, rect.2, rect.3);
    object.commit();
    self.commits += 1;
    trace::log(|| {
      format!(
        "[ime] commit #{}{}: surrounding {:?} cursor {} anchor {} rect {rect:?} cause {} purpose {purpose:?}",
        self.commits,
        if enabling { " (enable)" } else { "" },
        state.surrounding,
        state.cursor,
        state.anchor,
        if cause_input { "input-method" } else { "other" }
      )
    });
    self.sent = Some((state, rect));
  }

  /// Sends `disable` and its `commit`, counting that commit's `done` as seen: the compositor
  /// answers it only once an input method commits again.
  fn disable(&mut self) {
    if !self.enabled {
      return;
    }

    if let Some(object) = &self.object {
      object.disable();
      object.commit();
      self.commits += 1;
      self.done_serial += 1;
    }

    self.enabled = false;
    self.sent = None;
    self.pending = Pending::default();
    trace::log(|| format!("[ime] disabled (commit #{})", self.commits));
  }

  /// Handles an event of the text input, applying a `done` to the focused entry.
  pub fn event(&mut self, shell: &mut Shell, event: zwp_text_input_v3::Event) {
    match event {
      zwp_text_input_v3::Event::Enter { .. } => {
        trace::log(|| "[ime] enter".to_owned());
        self.entered = true;
        self.stale = true;
      }
      zwp_text_input_v3::Event::Leave { .. } => {
        trace::log(|| "[ime] leave".to_owned());
        self.disable();
        self.entered = false;
        shell.ime_preedit("", None, Vec::new());
      }
      zwp_text_input_v3::Event::PreeditString {
        text,
        cursor_begin,
        cursor_end,
      } => {
        self.pending.preedit = Some((text.unwrap_or_default(), cursor_begin, cursor_end));
      }
      zwp_text_input_v3::Event::CommitString { text } => {
        self.pending.commit = text;
      }
      zwp_text_input_v3::Event::DeleteSurroundingText {
        before_length,
        after_length,
      } => {
        self.pending.delete = Some((before_length, after_length));
      }
      zwp_text_input_v3::Event::Done { serial } => {
        let pending = std::mem::take(&mut self.pending);

        trace::log(|| {
          let secure = shell.focused_entry_is_secure();
          let shown = |text: &str| {
            if secure {
              crate::ime::marks(text)
            } else {
              text.to_owned()
            }
          };

          format!(
            "[ime] done serial {serial} (commits {}): preedit {:?} commit {:?} delete {:?}",
            self.commits,
            pending
              .preedit
              .as_ref()
              .map(|(text, begin, end)| (shown(text), begin, end)),
            pending.commit.as_deref().map(shown),
            pending.delete
          )
        });
        self.done_serial = serial;

        if !self.enabled {
          return;
        }

        let changed =
          pending.preedit.is_some() || pending.commit.is_some() || pending.delete.is_some();

        // The composition is removed before a deletion or a commit, and replaced in place
        // otherwise, so the entry never reports the text without it in between.
        let replaced = pending
          .preedit
          .as_ref()
          .is_some_and(|(text, ..)| !text.is_empty());

        if pending.delete.is_some() || pending.commit.is_some() || !replaced {
          shell.ime_preedit("", None, Vec::new());
        }

        if let Some((before, after)) = pending.delete {
          shell.ime_delete_surrounding(before as usize, after as usize);
        }

        if let Some(commit) = pending.commit {
          shell.ime_commit(&commit);
        }

        if let Some((text, begin, end)) = pending.preedit
          && !text.is_empty()
        {
          // A cursor off a character boundary or past the text is hidden, as the protocol
          // says; an end before the begin or inside a character (mutter 46 sends the anchor
          // in characters) leaves the caret at the begin.
          let valid = |index: i32| index >= 0 && text.is_char_boundary(index as usize);
          let cursor = valid(begin).then(|| {
            let end = if valid(end) && end >= begin {
              end
            } else {
              begin
            };

            begin as usize..end as usize
          });

          shell.ime_preedit(&text, cursor.or(Some(text.len()..text.len())), Vec::new());
        }

        if changed {
          self.cause_input = true;
          self.stale = true;
        }
      }
      _ => {}
    }
  }
}

/// What the Wayland side owning a [`TextInput`] dispatches its events to.
pub trait HasTextInput {
  /// The client and the shell its events apply to.
  fn text_input(&mut self) -> (&mut TextInput, &mut Shell);
}

impl<D: HasTextInput> sctk::dispatch2::Dispatch2<ZwpTextInputManagerV3, D> for Raw {
  fn event(
    &self,
    _: &mut D,
    _: &ZwpTextInputManagerV3,
    _: <ZwpTextInputManagerV3 as wayland_client::Proxy>::Event,
    _: &Connection,
    _: &QueueHandle<D>,
  ) {
  }
}

impl<D: HasTextInput> sctk::dispatch2::Dispatch2<ZwpTextInputV3, D> for Raw {
  fn event(
    &self,
    state: &mut D,
    _: &ZwpTextInputV3,
    event: zwp_text_input_v3::Event,
    _: &Connection,
    _: &QueueHandle<D>,
  ) {
    let (text_input, shell) = state.text_input();

    text_input.event(shell, event);
  }
}
