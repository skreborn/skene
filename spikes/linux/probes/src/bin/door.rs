//! The native door on real GTK: any widget is a measured leaf with accessibility facts, and a
//! library's controlled widget (Simple's date picker, a GtkCalendar) joins the controlled contract
//! through its ControlSink, acted on with real clicks.

use std::{cell::Cell, rc::Rc};

use probes::{atspi, line, result, run, settle, show, widget};
use skene_gui::{
  platform::linux::{
    NativeView,
    gtk::{self, prelude::*},
  },
  prelude::*,
};
use skene_simple_gui::{SimpleDate, SimpleDatePicker};

/// The label showing day `day` of the calendar's own month.
fn day(calendar: &gtk::Widget, day: u32) -> Option<gtk::Widget> {
  let mut stack = vec![calendar.clone()];
  let wanted = day.to_string();

  while let Some(widget) = stack.pop() {
    if let Some(label) = widget.downcast_ref::<gtk::Label>()
      && label.label() == wanted
      && !label.has_css_class("other-month")
    {
      return Some(widget);
    }

    let mut child = widget.first_child();

    while let Some(next) = child {
      child = next.next_sibling();
      stack.push(next);
    }
  }

  None
}

fn shown_date(calendar: &gtk::Widget) -> String {
  let date = calendar
    .downcast_ref::<gtk::Calendar>()
    .expect("a GtkCalendar")
    .date();

  format!(
    "{:04}-{:02}-{:02}",
    date.year(),
    date.month(),
    date.day_of_month()
  )
}

fn body() {
  // Any widget is a measured leaf: a calendar keeps its GTK minimum in a narrower box, and
  // carries the facts a view gives it.
  let native = NodeHandle::new();
  let shown = show(400, -1, || {
    Container::new().set_width(150).set_child(
      // GTK fixes a widget's role at construction and drops a name given to a `generic`
      // one, GtkCalendar's own: a library builds it in a role a name belongs to.
      NativeView::new(
        gtk::Calendar::builder()
          .accessible_role(gtk::AccessibleRole::Group)
          .build(),
      )
      .set_accessible_name("Due date")
      .set_handle(&native),
    )
  });
  let calendar = widget(&native);
  let minimum = calendar.measure(gtk::Orientation::Horizontal, -1).0 as f32;
  let width = shown.rect(&native).2;
  let dump = atspi();

  for line in &dump {
    println!("ATSPI {line}");
  }
  let named = line(&dump, &["\"Due date\""])
    .unwrap_or("(none)")
    .to_owned();

  result(
    "a native widget is laid out at least at its GTK minimum",
    width >= minimum && minimum > 150.,
    format!("allocated {width}, GTK minimum {minimum}, box 150"),
  );
  result(
    "a native widget takes a view's accessible name",
    named != "(none)",
    named,
  );

  shown.close();

  // A library's controlled calendar: a pick it accepts shows; one it refuses (weekends) is
  // repaired to the truth; a write from the signal reports nothing.
  let due = Signal::new("2026-10-14".to_owned());
  let picks = Rc::new(Cell::new(0));
  let [picker] = [NodeHandle::new()];
  let shown = {
    let picks = Rc::clone(&picks);

    show(400, -1, move || {
      Container::new().set_handle(&picker).set_child(
        SimpleDatePicker::new("Due")
          .derive_date(move || due.get())
          .handle_change(move |date| {
            picks.set(picks.get() + 1);

            if SimpleDate::parse(&date).is_some_and(|date| !date.is_weekend()) {
              due.set(date);
            }
          }),
      )
    })
  };
  let calendar = widget(&picker).first_child().expect("the calendar");
  let start = shown_date(&calendar);

  if let Some(twenty) = day(&calendar, 20) {
    shown.click(&twenty);
  }

  let accepted = (due.get(), shown_date(&calendar), picks.get());

  if let Some(seventeen) = day(&calendar, 17) {
    shown.click(&seventeen);
  }

  settle();

  let refused = (due.get(), shown_date(&calendar), picks.get());

  due.set("2026-11-03".to_owned());
  settle();

  let written = (shown_date(&calendar), picks.get());

  result(
    "the library's calendar shows the truth it was given",
    start == "2026-10-14",
    start,
  );
  result(
    "a click on Tuesday the 20th is reported and accepted",
    accepted == ("2026-10-20".to_owned(), "2026-10-20".to_owned(), 1),
    format!("{accepted:?}"),
  );
  result(
    "a click on Saturday the 17th is reported, refused, and repaired",
    refused == ("2026-10-20".to_owned(), "2026-10-20".to_owned(), 2),
    format!("{refused:?}"),
  );
  result(
    "a write from the signal shows and reports nothing",
    written == ("2026-11-03".to_owned(), 2),
    format!("{written:?}"),
  );

  shown.close();
}

fn main() -> std::process::ExitCode {
  run("door", body)
}
