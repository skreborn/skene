//! The toolkit's controls, wired to the seam: each reports the reader's change with its own
//! timing (`GtkSwitch` asks before applying through `state-set`, `GtkCheckButton` tells after
//! through `toggled`, `GtkScale` asks through `change-value`, an entry's text asks through
//! `insert-text` and `delete-text`, a text view's through its buffer's `insert-text` and
//! `delete-range`) and is brought back to the truth the verdict names. A write held while an
//! input method composed is shown before its commit is inserted, so the commit is the reader's
//! edit of the truth.

use std::{cell::Cell, rc::Rc};

use gtk::{glib, prelude::*};
use skene_foundation::{
  NodeId,
  seam::{
    content::{
      Choice, Content, Edit, Graphemes, PropValue, Range, Run, TextSelection, Unit, Verdict,
      is_writing, map_through, splice, writing,
    },
    node::ControlKind,
    record::{Prop, Resolved},
    report::{Answer, Report},
    tree,
  },
  value::FontStyle,
};

use crate::{Linux, report, slot};

/// `value` with `adjustment`'s bounds and step.
pub(crate) fn number_range(adjustment: &gtk::Adjustment, value: f64) -> Range {
  Range {
    value,
    minimum: adjustment.lower(),
    maximum: adjustment.upper(),
    step: adjustment.step_increment(),
  }
}

/// Writes a range in one call, so GTK never clamps a value against bounds it has yet to get.
pub(crate) fn configure(adjustment: &gtk::Adjustment, range: &Range) {
  adjustment.configure(
    range.value,
    range.minimum,
    range.maximum,
    range.step,
    range.step * 10.,
    0.,
  );
}

/// A dropdown's options, as its string list holds them.
pub(crate) fn options(dropdown: &gtk::DropDown) -> Vec<Rc<str>> {
  dropdown
    .model()
    .and_downcast::<gtk::StringList>()
    .map(|list| {
      (0..list.n_items())
        .filter_map(|index| list.string(index))
        .map(|text| Rc::from(text.as_str()))
        .collect()
    })
    .unwrap_or_default()
}

/// A dropdown's options with the one selected.
pub(crate) fn choice(dropdown: &gtk::DropDown) -> Choice {
  let selected = dropdown.selected();

  Choice {
    options: options(dropdown).into(),
    chosen: (selected != gtk::INVALID_LIST_POSITION).then_some(selected),
  }
}

/// Writes options and the choice in one call: GTK resets the choice when the options change.
pub(crate) fn write_choice(dropdown: &gtk::DropDown, truth: &Choice) {
  if options(dropdown)[..] != truth.options[..] {
    let strings: Vec<&str> = truth.options.iter().map(AsRef::as_ref).collect();

    dropdown.set_model(Some(&gtk::StringList::new(&strings)));
  }

  dropdown.set_selected(truth.chosen.unwrap_or(gtk::INVALID_LIST_POSITION));
}

/// A new toolkit control of `control` for `id`, its signals reporting through the seam.
pub(crate) fn control_widget(id: NodeId, control: ControlKind) -> gtk::Widget {
  match control {
    ControlKind::Switch => {
      let switch = gtk::Switch::new();

      switch.connect_state_set(move |switch, state| {
        if is_writing() {
          return glib::Propagation::Proceed;
        }

        match report(
          id,
          Report::Change {
            content: Content::On,
            proposed: PropValue::Bool(state),
            applied: false,
          },
        ) {
          Answer::Verdict(Verdict::Replace(_)) => {
            settle_on_layout(switch.upcast_ref(), id, Content::On);

            glib::Propagation::Stop
          }
          _ => glib::Propagation::Proceed,
        }
      });

      switch.upcast()
    }
    ControlKind::Slider => {
      let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0., 100., 1.);

      scale.set_draw_value(false);
      scale.connect_change_value(move |scale, _, value| {
        let adjustment = scale.adjustment();
        let proposed = number_range(
          &adjustment,
          value.clamp(adjustment.lower(), adjustment.upper()),
        );

        let answer = report(
          id,
          Report::Change {
            content: Content::Range,
            proposed: PropValue::Range(proposed),
            applied: false,
          },
        );
        let truth = match answer {
          Answer::Verdict(Verdict::Replace(PropValue::Range(truth))) => Some(truth),
          _ => {
            tree::controller::<Linux>(id, Content::Range).and_then(|controller| {
              match controller.truth() {
                PropValue::Range(truth) => Some(truth),
                _ => None,
              }
            })
          }
        };

        match truth {
          Some(truth) => {
            writing(|| configure(&adjustment, &truth));

            glib::Propagation::Stop
          }
          None => glib::Propagation::Proceed,
        }
      });

      scale.upcast()
    }
    ControlKind::Stepper => {
      let spin = gtk::SpinButton::with_range(0., 100., 1.);

      spin.connect_value_changed(move |spin| {
        if is_writing() {
          return;
        }

        let adjustment = spin.adjustment();

        if let Answer::Verdict(Verdict::Replace(PropValue::Range(truth))) = report(
          id,
          Report::Change {
            content: Content::Range,
            proposed: PropValue::Range(number_range(&adjustment, spin.value())),
            applied: true,
          },
        ) {
          writing(|| configure(&adjustment, &truth));
        }
      });

      spin.upcast()
    }
    ControlKind::Progress => gtk::ProgressBar::new().upcast(),
    ControlKind::TextField => text_field(id),
    ControlKind::TextEditor => text_editor(id),
    ControlKind::Button => {
      let button = gtk::Button::new();

      button.connect_clicked(move |_| {
        report(id, Report::Activate);
      });

      button.upcast()
    }
    ControlKind::Checkbox | ControlKind::Radio => {
      let check = gtk::CheckButton::new();

      check.connect_toggled(move |check| {
        if is_writing() {
          return;
        }

        if let Answer::Verdict(Verdict::Replace(PropValue::Bool(truth))) = report(
          id,
          Report::Change {
            content: Content::On,
            proposed: PropValue::Bool(check.is_active()),
            applied: true,
          },
        ) {
          writing(|| check.set_active(truth));
        }
      });

      check.upcast()
    }
    ControlKind::Picker => {
      let dropdown = gtk::DropDown::from_strings(&[]);

      dropdown.connect_selected_notify(move |dropdown| {
        if is_writing() {
          return;
        }

        if let Answer::Verdict(Verdict::Replace(PropValue::Choice(truth))) = report(
          id,
          Report::Change {
            content: Content::Choice,
            proposed: PropValue::Choice(choice(dropdown)),
            applied: true,
          },
        ) {
          writing(|| write_choice(dropdown, &truth));
        }
      });

      dropdown.upcast()
    }
  }
}

/// Settles `content` in the frame clock's next layout phase: GtkSwitch updates its accessible
/// state after its own signals return, and an idle paints one frame of the rejected state.
fn settle_on_layout(widget: &gtk::Widget, id: NodeId, content: Content) {
  let Some(clock) = widget.frame_clock() else {
    return;
  };
  let handler = Rc::new(Cell::new(None));
  let slot = Rc::clone(&handler);

  handler.set(Some(clock.connect_layout(move |clock| {
    if let Some(controller) = tree::controller::<Linux>(id, content) {
      controller.settle();
    }

    if let Some(handler) = slot.take() {
      clock.disconnect(handler);
    }
  })));
  clock.request_phase(gtk::gdk::FrameClockPhase::LAYOUT);
}

fn delegate(entry: &gtk::Entry) -> gtk::Text {
  entry
    .delegate()
    .and_downcast::<gtk::Text>()
    .expect("a GtkEntry edits through a GtkText")
}

/// The caret and the anchor, in code points.
pub(crate) fn caret_anchor(entry: &gtk::Entry) -> (usize, usize) {
  let text = delegate(entry);

  (
    text.property::<i32>("cursor-position").max(0) as usize,
    text.property::<i32>("selection-bound").max(0) as usize,
  )
}

fn select(entry: &gtk::Entry, text: &str, selection: TextSelection) {
  entry.select_region(
    Graphemes::offset(text, Unit::CodePoint, selection.start) as i32,
    Graphemes::offset(text, Unit::CodePoint, selection.end) as i32,
  );
}

/// Turns the shown text into `truth` as the smallest splice, the selection mapped through it
/// unless the truth names one.
pub(crate) fn splice_entry(entry: &gtk::Entry, truth: &Edit) {
  let old: String = entry.text().into();
  let (start, end, replacement) = splice(&old, &truth.text, Unit::CodePoint);
  let inserted = replacement.chars().count();
  let (caret, anchor) = caret_anchor(entry);

  writing(|| {
    if end > start {
      entry.delete_text(start as i32, end as i32);
    }

    if inserted > 0 {
      let mut position = start as i32;

      entry.insert_text(&replacement, &mut position);
    }

    match truth.selection {
      Some(selection) => select(entry, &truth.text, selection),
      None => entry.select_region(
        map_through(anchor, start, end, inserted) as i32,
        map_through(caret, start, end, inserted) as i32,
      ),
    }
  });
}

/// Reports a composition starting at once and ending once the input method's own handling of
/// it has returned.
fn composition(id: NodeId, composing: bool) {
  if composing {
    report(id, Report::Composition(true));
  } else {
    glib::idle_add_local_once(move || {
      report(id, Report::Composition(false));
    });
  }
}

/// The controller of `id`'s text where it holds a write for a composition that just ended.
fn held(id: NodeId) -> Option<std::rc::Rc<skene_foundation::seam::content::Controller<Linux>>> {
  tree::controller::<Linux>(id, Content::Edit).filter(|controller| controller.is_pending())
}

fn text_field(id: NodeId) -> gtk::Widget {
  let entry = gtk::Entry::new();
  let text = delegate(&entry);

  {
    let entry = entry.clone();

    text.connect_preedit_changed(move |_, preedit| {
      let now = !preedit.is_empty();

      if slot(entry.upcast_ref()).composing.replace(now) != now {
        composition(id, now);
      }
    });
  }

  {
    let entry = entry.clone();

    text.connect_insert_text(move |text, inserted, position| {
      if is_writing() {
        return;
      }

      if let Some(controller) = held(id) {
        text.stop_signal_emission_by_name("insert-text");
        controller.settle();

        let mut caret = text.position();

        entry.insert_text(inserted, &mut caret);
        *position = caret;

        return;
      }

      let at = (*position).max(0) as usize;

      if let Answer::Verdict(Verdict::Replace(PropValue::Edit(truth))) = report(
        id,
        Report::Edit {
          start: at,
          end: at,
          replacement: inserted.into(),
          applied: false,
        },
      ) {
        text.stop_signal_emission_by_name("insert-text");

        let mut proposed: Vec<char> = entry.text().chars().collect();
        let count = inserted.chars().count();

        proposed.splice(at..at, inserted.chars());

        let proposed: String = proposed.into_iter().collect();
        let (start, end, replacement) = splice(&proposed, &truth.text, Unit::CodePoint);
        let caret = map_through(at + count, start, end, replacement.chars().count());

        splice_entry(&entry, &truth);

        if truth.selection.is_none() {
          *position = caret as i32;
          writing(|| entry.set_position(caret as i32));
        }
      }
    });
  }

  {
    let entry = entry.clone();

    text.connect_delete_text(move |text, start, end| {
      if is_writing() {
        return;
      }

      let end = if end < 0 {
        entry.text().chars().count()
      } else {
        end as usize
      };

      if let Answer::Verdict(Verdict::Replace(PropValue::Edit(truth))) = report(
        id,
        Report::Edit {
          start: start.max(0) as usize,
          end,
          replacement: "".into(),
          applied: false,
        },
      ) {
        text.stop_signal_emission_by_name("delete-text");
        splice_entry(&entry, &truth);
      }
    });
  }

  for property in ["cursor-position", "selection-bound"] {
    let entry = entry.clone();

    text.connect_notify_local(Some(property), move |_, _| {
      if is_writing() {
        return;
      }

      let (caret, anchor) = caret_anchor(&entry);

      if let Answer::Verdict(Verdict::Replace(PropValue::Edit(truth))) =
        report(id, Report::Selection { anchor, caret })
      {
        writing(|| splice_entry(&entry, &truth));
      }
    });
  }

  entry.connect_activate(move |_| {
    report(id, Report::Activate);
  });

  entry.upcast()
}

/// A text buffer's whole text.
pub(crate) fn buffer_text(buffer: &gtk::TextBuffer) -> String {
  buffer
    .text(&buffer.start_iter(), &buffer.end_iter(), true)
    .into()
}

/// The caret and the anchor of a text view, in code points.
fn view_caret_anchor(buffer: &gtk::TextBuffer) -> (usize, usize) {
  let offset = |mark: &gtk::TextMark| buffer.iter_at_mark(mark).offset().max(0) as usize;

  (
    offset(&buffer.get_insert()),
    offset(&buffer.selection_bound()),
  )
}

/// Turns a text view's text into `truth` as the smallest splice, outside any user action.
pub(crate) fn splice_view(view: &gtk::TextView, truth: &Edit) {
  let buffer = view.buffer();
  let old = buffer_text(&buffer);
  let (start, end, replacement) = splice(&old, &truth.text, Unit::CodePoint);
  let inserted = replacement.chars().count();
  let (caret, anchor) = view_caret_anchor(&buffer);

  writing(|| {
    if end > start {
      buffer.delete(
        &mut buffer.iter_at_offset(start as i32),
        &mut buffer.iter_at_offset(end as i32),
      );
    }

    if inserted > 0 {
      buffer.insert(&mut buffer.iter_at_offset(start as i32), &replacement);
    }

    let (anchor, caret) = match truth.selection {
      Some(selection) => (
        Graphemes::offset(&truth.text, Unit::CodePoint, selection.start),
        Graphemes::offset(&truth.text, Unit::CodePoint, selection.end),
      ),
      None => (
        map_through(anchor, start, end, inserted),
        map_through(caret, start, end, inserted),
      ),
    };

    buffer.select_range(
      &buffer.iter_at_offset(caret as i32),
      &buffer.iter_at_offset(anchor as i32),
    );
  });
}

/// A text view: each edit reported before its buffer applies it (`insert-text`,
/// `delete-range`), undo and redo included; where the truth is another text the edit is stopped
/// and the truth shown as the smallest splice. Each move of the caret or the selection is
/// reported (`mark-set` of `insert` or `selection_bound`).
fn text_editor(id: NodeId) -> gtk::Widget {
  let view = gtk::TextView::new();
  let buffer = view.buffer();

  view.set_wrap_mode(gtk::WrapMode::WordChar);

  {
    let view = view.clone();

    buffer.connect_insert_text(move |buffer, location, inserted| {
      if is_writing() {
        return;
      }

      if let Some(controller) = held(id) {
        buffer.stop_signal_emission_by_name("insert-text");
        controller.settle();
        buffer.insert(&mut buffer.iter_at_mark(&buffer.get_insert()), inserted);

        return;
      }

      let at = location.offset().max(0) as usize;

      if let Answer::Verdict(Verdict::Replace(PropValue::Edit(truth))) = report(
        id,
        Report::Edit {
          start: at,
          end: at,
          replacement: inserted.into(),
          applied: false,
        },
      ) {
        buffer.stop_signal_emission_by_name("insert-text");

        let mut proposed: Vec<char> = buffer_text(buffer).chars().collect();
        let count = inserted.chars().count();

        proposed.splice(at..at, inserted.chars());

        let proposed: String = proposed.into_iter().collect();
        let (start, end, replacement) = splice(&proposed, &truth.text, Unit::CodePoint);
        let caret = map_through(at + count, start, end, replacement.chars().count());

        splice_view(&view, &truth);

        if truth.selection.is_none() {
          writing(|| buffer.place_cursor(&buffer.iter_at_offset(caret as i32)));
        }
      }
    });
  }

  {
    let view = view.clone();

    buffer.connect_delete_range(move |buffer, start, end| {
      if is_writing() {
        return;
      }

      let (start, end) = (start.offset().max(0) as usize, end.offset().max(0) as usize);

      if let Answer::Verdict(Verdict::Replace(PropValue::Edit(truth))) = report(
        id,
        Report::Edit {
          start: start.min(end),
          end: start.max(end),
          replacement: "".into(),
          applied: false,
        },
      ) {
        buffer.stop_signal_emission_by_name("delete-range");
        splice_view(&view, &truth);
      }
    });
  }

  {
    let view = view.clone();

    buffer.connect_mark_set(move |buffer, _, mark| {
      if is_writing() || (*mark != buffer.get_insert() && *mark != buffer.selection_bound()) {
        return;
      }

      let (caret, anchor) = view_caret_anchor(buffer);

      if let Answer::Verdict(Verdict::Replace(PropValue::Edit(truth))) =
        report(id, Report::Selection { anchor, caret })
      {
        splice_view(&view, &truth);
      }
    });
  }

  {
    let view = view.clone();

    view.clone().connect_preedit_changed(move |_, preedit| {
      let now = !preedit.is_empty();

      if slot(view.upcast_ref()).composing.replace(now) != now {
        composition(id, now);
      }
    });
  }

  view.upcast()
}

fn escape(text: &str) -> String {
  glib::markup_escape_text(text).into()
}

/// Rich text as Pango markup, a link run an `<a href>` GTK's label follows.
pub(crate) fn markup(runs: &[Run]) -> String {
  runs
    .iter()
    .map(|run| {
      let mut attributes = String::new();

      for (prop, value) in run.props.iter() {
        match (prop, value) {
          (Prop::Color, Resolved::Color(color)) => attributes.push_str(&format!(
            " foreground=\"#{:02x}{:02x}{:02x}\"",
            color.red, color.green, color.blue
          )),
          (Prop::FontWeight, Resolved::FontWeight(weight)) => {
            attributes.push_str(&format!(" weight=\"{}\"", weight.0));
          }
          (Prop::FontSize, Resolved::Number(size)) => {
            attributes.push_str(&format!(" size=\"{}\"", (size * 0.75 * 1024.) as i32));
          }
          (Prop::FontStyle, Resolved::FontStyle(FontStyle::Italic)) => {
            attributes.push_str(" style=\"italic\"");
          }
          (Prop::Decoration, Resolved::TextDecoration(decoration)) => {
            if decoration.underline {
              attributes.push_str(" underline=\"single\"");
            }

            if decoration.line_through {
              attributes.push_str(" strikethrough=\"true\"");
            }
          }
          _ => {}
        }
      }

      let text = if attributes.is_empty() {
        escape(&run.text)
      } else {
        format!("<span{attributes}>{}</span>", escape(&run.text))
      };

      match &run.destination {
        Some(destination) => format!("<a href=\"{}\">{text}</a>", escape(destination)),
        None => text,
      }
    })
    .collect()
}
