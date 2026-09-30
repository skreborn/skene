//! A user's gestures on the headless platform, each the sequence of events a browser
//! dispatches for it, through the `trigger_` seams.
//!
//! A pointer acts at the center of the size last reported for the node
//! ([`trigger_size`](crate::trigger_size)), or at its corner where none was; a key acts
//! where focus is.

use skene_core::{
  Key,
  KeyEvent,
  NamedKey,
  NativeElement,
  NodeId,
  PointerButton,
  PointerEvent,
  Scene,
  TextSelection,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
  Headless,
  end_turn,
  focus_order,
  store::Kind,
  trigger_blur,
  trigger_change,
  trigger_click,
  trigger_double_click,
  trigger_focus,
  trigger_key_down,
  trigger_key_up,
  trigger_pointer_down,
  trigger_pointer_enter,
  trigger_pointer_leave,
  trigger_pointer_move,
  trigger_pointer_up,
  trigger_selection_change,
  with_record,
};

/// The primary mouse at the center of the size last reported for `node`.
fn pointer_at(node: NodeId) -> PointerEvent {
  let size = with_record(node, |record| record.size).flatten();
  let (x, y) = size.map_or((0., 0.), |size| (size.width / 2., size.height / 2.));

  PointerEvent::mouse(x, y)
}

/// Moves the pointer over `node` and within it.
pub fn hover(node: NodeId) {
  trigger_pointer_enter(node);

  let _ = trigger_pointer_move(node, &pointer_at(node));
}

/// Moves the pointer off `node`.
pub fn unhover(node: NodeId) {
  trigger_pointer_leave(node);
}

/// Clicks `node` with the primary button.
///
/// The pointer moves over it and presses, focus moves to the nearest node at or above it
/// that takes focus, or off the focused node where none does, and the pointer releases
/// and clicks.
pub fn click(node: NodeId) {
  hover(node);
  press_and_release(node);

  let _ = trigger_click(node);
}

/// Double-clicks `node`: a click, then a second press and release, a click counting
/// two and a double click.
pub fn double_click(node: NodeId) {
  click(node);
  press_and_release(node);

  let _ = trigger_double_click(node);
}

/// Presses and releases the primary button on `node`, the press moving focus.
fn press_and_release(node: NodeId) {
  let press = pointer_at(node).with_press(PointerButton::Primary);
  let _ = trigger_pointer_down(node, &press);
  let target = Scene::<Headless>::with(|scene| scene.ancestry(node))
    .into_iter()
    .find(|ancestor| focus_order(*ancestor).is_some());

  match target {
    Some(target) => trigger_focus(target),
    None => {
      if let Some(focused) = Scene::<Headless>::with(|scene| scene.focused()) {
        trigger_blur(focused);
      }
    }
  }

  let mut release = press;

  release.buttons.remove(PointerButton::Primary);

  let _ = trigger_pointer_up(node, &release);
}

/// Presses and releases the key `event` describes where focus is, or at the first tree's
/// root where nothing is focused.
///
/// A response no handler suppressed follows the press: `Tab` moves focus, and in a focused
/// text entry a typed character replaces the selection, `Enter` a newline in a multi-line
/// one, and `Backspace` and `Delete` remove the selection or the cluster beside the caret.
pub fn press_key(event: &KeyEvent) {
  end_turn();

  let target = Scene::<Headless>::with(|scene| scene.focused().or_else(|| scene.roots().next()));
  let Some(target) = target
  else {
    return;
  };

  if !trigger_key_down(target, event).platform {
    edit(target, event);
  }

  let _ = trigger_key_up(target, event);
}

/// Clicks the text entry `node` and types `text` into it, one grapheme cluster at a time,
/// each a key press, a newline pressing `Enter`.
pub fn type_text(node: NodeId, text: &str) {
  click(node);

  for cluster in text.graphemes(true) {
    let event = match cluster {
      "\n" | "\r\n" => KeyEvent::new("Enter"),
      _ => KeyEvent {
        key: Key::Character(cluster.to_owned()),
        ..KeyEvent::new(cluster)
      },
    };

    press_key(&event);
  }
}

/// Applies the response of the key `event` to `node` where it is a text entry taking
/// edits: a typed character replaces the selection, `Enter` a newline in a multi-line
/// entry, and `Backspace` and `Delete` remove the selection or the cluster beside the
/// caret.
fn edit(node: NodeId, event: &KeyEvent) {
  let modifiers = event.modifiers;

  if modifiers.ctrl
    || modifiers.alt
    || modifiers.meta
    || !Scene::<Headless>::read(|scene| scene.is_editable(node))
  {
    return;
  }

  let entry = with_record(node, |record| match &record.kind {
    Kind::Control(props) if props.native.is_some_and(NativeElement::is_text_entry) => {
      Some((props.native, record.value.clone(), record.selection))
    }
    _ => None,
  })
  .flatten();
  let Some((native, value, selection)) = entry
  else {
    return;
  };
  let clusters: Vec<&str> = value.graphemes(true).collect();
  let count = clusters.len();
  let (start, end) = (selection.min().min(count), selection.max().min(count));
  let (from, to, insert) = match &event.key {
    Key::Character(text) => (start, end, text.as_str()),
    Key::Named(NamedKey::Enter) if native == Some(NativeElement::TextEditor) => (start, end, "\n"),
    Key::Named(NamedKey::Backspace) if start == end => (start.saturating_sub(1), end, ""),
    Key::Named(NamedKey::Delete) if start == end => (start, (end + 1).min(count), ""),
    Key::Named(NamedKey::Backspace | NamedKey::Delete) => (start, end, ""),
    Key::Named(_) => return,
  };

  if from == to && insert.is_empty() {
    return;
  }

  let next = [&clusters[..from].concat(), insert, &clusters[to..].concat()].concat();

  trigger_change(node, &next);
  trigger_selection_change(
    node,
    TextSelection::collapsed(from + insert.graphemes(true).count()),
  );
}
