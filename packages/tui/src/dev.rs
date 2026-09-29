//! Dev-loop state: restored from the file the launcher hands over at boot, recorded to
//! it at a swap, and the launcher's notices over its control pipe.
//!
//! The handshake pairs with the `skene` CLI's `attach`, which spawns the app: it names
//! the descriptor of an unnamed state file in `SKENE_DEV_STATE` and the read end of its
//! pipe in `SKENE_DEV_CONTROL`. The app restores the file's snapshot before mounting and
//! empties the file; on the pipe it hears `notice <text>` (a rebuild failure, shown in a
//! dismissable modal, `\n` standing for a line break; bare `notice` clears it) and
//! `swap <path>` (a rebuilt binary), on which it records its snapshot to the file,
//! restores the terminal and becomes the new binary in place, so the launcher, the
//! terminal, the file and the pipe carry over and no server holds any state. Where the
//! process cannot be replaced it exits with [`SWAP_EXIT`] and the launcher spawns the
//! binary. Without the variables, or with a state descriptor naming no regular file,
//! every call is a no-op.

use std::{
  cell::RefCell,
  fs::File,
  io::{Read as _, Seek as _, SeekFrom, Write as _},
  mem::ManuallyDrop,
  os::fd::{FromRawFd as _, RawFd},
  path::{Path, PathBuf},
};

use skene_core::{Color, HasChildren, Scene};
use skene_reactive::{Get, Position, Set, Signal};

use crate::{Column, IntoView as _, Overlay, Text, Tui, TuiText, store::Handle};

/// The exit status telling the launcher the app stopped for a swap it could not
/// perform in place.
pub(crate) const SWAP_EXIT: i32 = 75;

/// The snapshot tag carrying every scrolled node's offsets, by position.
const SCROLL_TAG: &str = "skene-tui:scroll";

/// The snapshot tag of the focused node's position and, for a text entry, its caret
/// and selection anchor.
const FOCUS_TAG: &str = "skene-tui:focus";

/// A focused entry's caret and selection anchor.
type Caret = (usize, Option<usize>);

/// The dev channel, present when the launcher started the app.
struct Channel {
  notice: Signal<Option<String>>,
  state: RawFd,
}

thread_local! {
  static CHANNEL: RefCell<Option<Channel>> = const { RefCell::new(None) };
}

/// The state file the launcher handed over at `fd`, borrowed so the descriptor stays
/// open for every binary the process becomes; `None` where `fd` names no regular
/// file.
fn state_file(fd: RawFd) -> Option<ManuallyDrop<File>> {
  if fd <= 2 {
    return None;
  }

  // SAFETY: the launcher opened `fd` for this process, and nothing closes it: the file
  // is never dropped.
  let file = ManuallyDrop::new(unsafe { File::from_raw_fd(fd) });

  file
    .metadata()
    .is_ok_and(|metadata| metadata.is_file())
    .then_some(file)
}

/// Restores the snapshot the state file holds, when the launcher handed one over,
/// empties it, and returns the control pipe to listen on.
pub(crate) fn install() -> Option<RawFd> {
  let state: RawFd = std::env::var("SKENE_DEV_STATE").ok()?.parse().ok()?;
  let mut file = state_file(state)?;
  let mut bytes = Vec::new();

  if file.seek(SeekFrom::Start(0)).is_ok()
    && file.read_to_end(&mut bytes).is_ok()
    && !bytes.is_empty()
  {
    skene_reactive::restore(&bytes);

    let _ = file.set_len(0);
  }

  CHANNEL.with(|channel| {
    *channel.borrow_mut() = Some(Channel {
      notice: Signal::opaque(None),
      state,
    });
  });

  std::env::var("SKENE_DEV_CONTROL")
    .ok()
    .and_then(|value| value.parse().ok())
}

/// Acts on one line the launcher wrote: a notice updates the modal, and a swap names
/// the binary to become.
pub(crate) fn handle(line: &str) -> Option<PathBuf> {
  if let Some(path) = line.strip_prefix("swap ") {
    return Some(PathBuf::from(path.trim()));
  }

  if let Some(text) = line.strip_prefix("notice") {
    let text = text.trim().replace("\\n", "\n");
    let notice = (!text.is_empty()).then_some(text);

    CHANNEL.with(|channel| {
      if let Some(channel) = channel.borrow().as_ref() {
        channel.notice.set(notice);
      }
    });
  }

  None
}

/// Records the snapshot to the state file and becomes the binary at `path`: in place
/// where the process can be replaced, else by exiting with [`SWAP_EXIT`] for the
/// launcher to spawn it. The terminal must already be restored.
pub(crate) fn swap(path: &Path) -> ! {
  let state = CHANNEL.with(|channel| channel.borrow().as_ref().map(|channel| channel.state));

  if let Some(mut file) = state.and_then(state_file) {
    let snapshot = skene_reactive::snapshot();
    let _ = file
      .set_len(0)
      .and_then(|()| file.seek(SeekFrom::Start(0)))
      .and_then(|_| file.write_all(&snapshot));
  }

  #[cfg(unix)]
  {
    use std::os::unix::process::CommandExt as _;

    let error = std::process::Command::new(path).exec();

    eprintln!("could not become {}: {error}", path.display());
  }

  std::process::exit(SWAP_EXIT)
}

/// The snapshot position of the node `handle` realizes.
fn position_of(handle: Handle) -> Option<Position> {
  let node = crate::try_with_store(|store| store.record(handle)?.node)??;

  Scene::<Tui>::with(|scene| scene.record(node).map(|record| record.position().clone()))
}

/// The handle realizing the node at `position`, if it is live.
fn handle_at(position: &Position) -> Option<Handle> {
  Scene::<Tui>::with(|scene| scene.node_at(position).and_then(|node| scene.sink(node)))
}

/// Every scrolled node's offsets by position, in creation order: the dev loop's carry
/// across a restart.
fn scroll_snapshot() -> Option<Vec<(Position, u32, u32)>> {
  let scrolled: Vec<(Handle, u32, u32)> = crate::try_with_store(|store| {
    store
      .handles()
      .into_iter()
      .filter_map(|handle| {
        let (x, y) = store.record(handle)?.scroll;

        (x > 0 || y > 0).then_some((handle, x, y))
      })
      .collect()
  })?;

  Some(
    scrolled
      .into_iter()
      .filter_map(|(handle, x, y)| Some((position_of(handle)?, x, y)))
      .collect(),
  )
}

/// The focused node's position and, for a text entry, its caret and selection anchor.
fn focus_snapshot() -> Option<(Position, Option<Caret>)> {
  let (focused, caret) = crate::try_with_store(|store| {
    let focused = store.focused?;
    let caret = store
      .record(focused)
      .and_then(|record| record.kind.field())
      .map(|field| (field.caret, field.anchor));

    Some((focused, caret))
  })??;

  Some((position_of(focused)?, caret))
}

/// Registers the platform's scroll offsets and focused node as snapshot providers; the
/// entries ride every snapshot alongside the tagged signals.
pub(crate) fn provide_restoration() {
  skene_reactive::provide(SCROLL_TAG, || {
    scroll_snapshot()
      .filter(|entries| !entries.is_empty())
      .map(|entries| skene_reactive::encode(&entries))
  });
  skene_reactive::provide(FOCUS_TAG, || {
    focus_snapshot().map(|focus| skene_reactive::encode(&focus))
  });
}

/// Adopts the focus and scroll offsets a restore staged, once the tree is mounted:
/// entries no live node carries are dropped, and reads clamp to the next layout.
pub fn adopt_restoration() {
  adopt_focus();
  adopt_scroll();
}

/// Adopts the focused node a restore staged, and its caret and selection where the
/// node is a text entry.
fn adopt_focus() {
  let Some((position, caret)) = skene_reactive::staged(FOCUS_TAG)
    .and_then(|bytes| skene_reactive::decode::<(Position, Option<Caret>)>(&bytes))
  else {
    return;
  };
  let Some(handle) = handle_at(&position)
  else {
    return;
  };

  crate::interaction::set_focus(Some(handle));

  if let Some((caret, anchor)) = caret {
    crate::with_store(|store| {
      if let Some(field) = store
        .record_mut(handle)
        .and_then(|record| record.kind.field_mut())
      {
        field.select(anchor.unwrap_or(caret), caret);
      }

      store.settle_entry(handle);
      store.damage(handle);
    });
  }
}

/// Adopts the scroll offsets a restore staged.
fn adopt_scroll() {
  let Some(bytes) = skene_reactive::staged(SCROLL_TAG)
  else {
    return;
  };
  let Some(entries) = skene_reactive::decode::<Vec<(Position, u32, u32)>>(&bytes)
  else {
    return;
  };
  let restored: Vec<(Handle, (u32, u32))> = entries
    .into_iter()
    .filter_map(|(position, x, y)| Some((handle_at(&position)?, (x, y))))
    .collect();

  // The scroll handlers hear the restored offsets, so a lazy column rebuilds its
  // window at the resumed position.
  for (handle, offsets) in restored {
    crate::interaction::scroll_to(handle, offsets);
  }
}

/// The rebuild-notice signal, present when the launcher started the app.
pub(crate) fn notice_signal() -> Option<Signal<Option<String>>> {
  CHANNEL.with(|channel| channel.borrow().as_ref().map(|channel| channel.notice))
}

/// The dev overlay presenting the launcher's rebuild failures: a modal over the app
/// while `notice` holds one, dismissed back to `None` by Escape or a press outside.
#[must_use]
pub fn notice_view(notice: Signal<Option<String>>) -> impl crate::View {
  let dark = crate::with_store(|store| store.dark());
  let fill = if dark {
    Color::srgb(0., 0., 0.)
  }
  else {
    Color::srgb(1., 1., 1.)
  };

  Overlay::modal()
    .derive_open(move || notice.get().is_some())
    .handle_dismiss(move || notice.set(None))
    .set_color(fill)
    .set_border_width(skene_core::Inset::all(1))
    .set_border_color(Color::srgb(0.8, 0.2, 0.2))
    .set_padding(crate::Inset {
      top: 1,
      right: 2,
      bottom: 1,
      left: 2,
    })
    .set_max_width(76)
    .set_child(
      Column::new().set_spacing(1).set_children((
        Text::new("Rebuild failed")
          .set_bold(true)
          .set_color(Color::srgb(0.8, 0.2, 0.2))
          .into_view(),
        Text::derive(move || notice.get().unwrap_or_default())
          .set_selectable(true)
          .into_view(),
        Text::new("Esc to dismiss")
          .set_color(Color::srgb(0.5, 0.5, 0.5))
          .into_view(),
      )),
    )
}
