//! The interests a node watches ([`skene_foundation::seam::report::Interest`]), each one GTK
//! event controller: hover as a state, pointer and key events, focus.

use std::cell::Cell;

use gtk::{glib, glib::translate::IntoGlib, prelude::*};
use skene_foundation::{
  NodeId, State,
  seam::report::{KeyEvent, Modifiers, PointerEvent, PointerPhase, Report},
};

use crate::{handled, nearest_node, node_of, report};

thread_local! {
  static LAST_EVENT: Cell<Option<(u32, u32)>> = const { Cell::new(None) };
}

/// Whether this event was reported already, by a deeper node's controller.
fn reported(time: u32, detail: u32) -> bool {
  LAST_EVENT.with(|last| last.replace(Some((time, detail))) == Some((time, detail)))
}

/// A key's name as a browser's `key` names it.
fn key_name(key: gtk::gdk::Key) -> String {
  if let Some(character) = key.to_unicode().filter(|character| !character.is_control()) {
    return character.to_string();
  }

  let name = key.name().map(|name| name.to_string()).unwrap_or_default();

  match name.as_str() {
    "Control_L" | "Control_R" => "Control",
    "Shift_L" | "Shift_R" => "Shift",
    "Alt_L" | "Alt_R" => "Alt",
    "Super_L" | "Super_R" => "Meta",
    "Return" | "KP_Enter" => "Enter",
    "BackSpace" => "Backspace",
    "Left" => "ArrowLeft",
    "Right" => "ArrowRight",
    "Up" => "ArrowUp",
    "Down" => "ArrowDown",
    _ => return name,
  }
  .to_owned()
}

const fn modifiers(state: gtk::gdk::ModifierType) -> Modifiers {
  Modifiers {
    shift: state.contains(gtk::gdk::ModifierType::SHIFT_MASK),
    control: state.contains(gtk::gdk::ModifierType::CONTROL_MASK),
    alt: state.contains(gtk::gdk::ModifierType::ALT_MASK),
    meta: state.contains(gtk::gdk::ModifierType::SUPER_MASK),
  }
}

/// Installs the controller reporting this interest.
pub(crate) fn watch_hover(widget: &gtk::Widget, id: NodeId) {
  let motion = gtk::EventControllerMotion::new();

  motion.connect_enter(move |_, _, _| {
    report(
      id,
      Report::State {
        state: State::Hovered,
        holds: true,
      },
    );
  });
  motion.connect_leave(move |_| {
    report(
      id,
      Report::State {
        state: State::Hovered,
        holds: false,
      },
    );
  });

  widget.add_controller(motion);
}

/// Installs the controller reporting this interest.
pub(crate) fn watch_pointer(widget: &gtk::Widget) {
  let click = gtk::GestureClick::new();

  click.set_button(0);

  let pointer = |phase: PointerPhase| {
    move |gesture: &gtk::GestureClick, _: i32, x: f64, y: f64| {
      if reported(gesture.current_event_time(), phase as u32) {
        return;
      }

      let Some(widget) = gesture.widget() else {
        return;
      };
      let target = widget
        .pick(x, y, gtk::PickFlags::DEFAULT)
        .and_then(|picked| nearest_node(&picked))
        .or_else(|| node_of(&widget));
      let Some(target) = target else {
        return;
      };
      let answer = report(
        target,
        Report::Pointer(PointerEvent {
          phase,
          x: x as f32,
          y: y as f32,
          button: gesture.current_button().saturating_sub(1) as u8,
        }),
      );

      if handled(&answer) {
        gesture.set_state(gtk::EventSequenceState::Claimed);
      }
    }
  };

  click.connect_pressed(pointer(PointerPhase::Down));
  click.connect_released(pointer(PointerPhase::Up));
  widget.add_controller(click);
}

/// Installs the controller reporting this interest.
pub(crate) fn watch_keys(widget: &gtk::Widget) {
  let keys = gtk::EventControllerKey::new();

  keys.connect_key_pressed(|controller, key, code, state| {
    if reported(controller.current_event_time(), key.into_glib()) {
      return glib::Propagation::Proceed;
    }

    let Some(widget) = controller.widget() else {
      return glib::Propagation::Proceed;
    };
    let focus = widget.root().and_then(|root| root.focus());
    let Some(target) = focus
      .as_ref()
      .and_then(nearest_node)
      .or_else(|| node_of(&widget))
    else {
      return glib::Propagation::Proceed;
    };
    let answer = report(
      target,
      Report::Key(KeyEvent {
        key: key_name(key),
        code: code.to_string(),
        down: true,
        repeat: false,
        modifiers: modifiers(state),
      }),
    );

    if handled(&answer) {
      glib::Propagation::Stop
    } else {
      glib::Propagation::Proceed
    }
  });
  widget.add_controller(keys);
}

/// Installs the controller reporting this interest.
pub(crate) fn watch_focus(widget: &gtk::Widget, id: NodeId) {
  let focus = gtk::EventControllerFocus::new();

  focus.connect_enter(move |_| {
    report(id, Report::Focus(true));
  });
  focus.connect_leave(move |_| {
    report(id, Report::Focus(false));
  });

  widget.add_controller(focus);
}
