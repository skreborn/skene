//! The controls the realization draws: what each holds (its contents), how big it is, how it
//! looks in each state, and what a press, a drag or a key does to it.
//!
//! What a key, a click, a press or a reader's action does to a control is its behavior's, from
//! the foundation ([`skene_foundation::behavior`]): the keyboard tables live there, once for
//! every platform that draws its controls. A control reports what the reader did and is
//! answered at once ([`Act`] crosses to the shell, which reports it and applies the verdict);
//! it never applies a change itself. Text entries are [`crate::entry`]'s.

use std::rc::Rc;

use skene_foundation::{
  Color, Corners, State,
  behavior::{Activate, Behavior as _, Popup, PopupAct, Response, Slider, Toggle, ToggleKind},
  layout::{Axis, Measurement, Rect},
  seam::{
    access::{AccessFacts, Action},
    content::{Choice, Content, Edit, Pixels, PropValue, Range, Run},
    node::ControlKind,
    report::KeyEvent,
  },
  text::{AttributedText, Laid as _},
  value::BorderStyle,
};
use vello_cpu::kurbo::BezPath;

use crate::{
  look::RunProps,
  node::Drawn,
  paint::{Canvas, Marks, focus_ring},
  palette::{DEFAULT_SIZE, Palette},
  text::TextCell,
};

/// A node's content values, as Skene last wrote them or the reader last changed them.
#[derive(Clone, Default, Debug)]
pub struct Contents {
  /// A text's plain text.
  pub text: Option<Rc<str>>,
  /// A text's rich text.
  pub runs: Option<Rc<[Run]>>,
  /// A control's caption.
  pub caption: Option<Rc<str>>,
  /// Whether it is on.
  pub on: bool,
  /// Whether it is mixed.
  pub mixed: bool,
  /// A value on a range.
  pub range: Option<Range>,
  /// Options with the chosen one.
  pub choice: Option<Choice>,
  /// A text entry's text and selection.
  pub edit: Option<Edit>,
  /// What an empty entry shows.
  pub placeholder: Option<Rc<str>>,
  /// Whether a button is the prominent one.
  pub prominent: bool,
  /// Whether it takes no input.
  pub disabled: bool,
  /// Whether its text is read only.
  pub read_only: bool,
  /// Whether it hides what is typed.
  pub secure: bool,
  /// The keyboard it asks the input method for, by name.
  pub keyboard: Option<Rc<str>>,
  /// The picture an image shows.
  pub source: Option<Rc<str>>,
  /// Where an acting box leads.
  pub destination: Option<Rc<str>>,
  /// What a painted node shows.
  pub pixels: Option<Pixels>,
}

impl Contents {
  /// Shows `value` as `content`.
  pub fn write(&mut self, content: Content, value: &PropValue) {
    match (content, value) {
      (Content::Text, PropValue::Text(text)) => self.text = Some(Rc::clone(text)),
      (Content::Runs, PropValue::Runs(runs)) => self.runs = Some(Rc::clone(runs)),
      (Content::Caption, PropValue::Text(text)) => self.caption = Some(Rc::clone(text)),
      (Content::On, PropValue::Bool(on)) => self.on = *on,
      (Content::Mixed, PropValue::Bool(mixed)) => self.mixed = *mixed,
      (Content::Range, PropValue::Range(range)) => self.range = Some(*range),
      (Content::Choice, PropValue::Choice(choice)) => self.choice = Some(choice.clone()),
      (Content::Edit, PropValue::Edit(edit)) => self.edit = Some(edit.clone()),
      (Content::Placeholder, PropValue::Text(text)) => self.placeholder = Some(Rc::clone(text)),
      (Content::Prominent, PropValue::Bool(prominent)) => self.prominent = *prominent,
      (Content::Enabled, PropValue::Bool(enabled)) => self.disabled = !enabled,
      (Content::ReadOnly, PropValue::Bool(read_only)) => self.read_only = *read_only,
      (Content::Secure, PropValue::Bool(secure)) => self.secure = *secure,
      (Content::Keyboard, PropValue::Text(keyboard)) => self.keyboard = Some(Rc::clone(keyboard)),
      (Content::Source, PropValue::Text(text)) => self.source = Some(Rc::clone(text)),
      (Content::Destination, PropValue::Text(text)) => self.destination = Some(Rc::clone(text)),
      (Content::Pixels, PropValue::Pixels(pixels)) => self.pixels = Some(pixels.clone()),
      _ => {}
    }
  }

  /// The value shown as `content`.
  pub fn read(&self, content: Content) -> Option<PropValue> {
    let text = |text: &Option<Rc<str>>| text.clone().map(PropValue::Text);

    match content {
      Content::Text => text(&self.text),
      Content::Runs => self.runs.clone().map(PropValue::Runs),
      Content::Caption => text(&self.caption),
      Content::On => Some(PropValue::Bool(self.on)),
      Content::Mixed => Some(PropValue::Bool(self.mixed)),
      Content::Range => self.range.map(PropValue::Range),
      Content::Choice => self.choice.clone().map(PropValue::Choice),
      Content::Edit => Some(PropValue::Edit(
        self
          .edit
          .clone()
          .unwrap_or_else(|| Edit::text(Rc::<str>::from(""))),
      )),
      Content::Placeholder => text(&self.placeholder),
      Content::Prominent => Some(PropValue::Bool(self.prominent)),
      Content::Enabled => Some(PropValue::Bool(!self.disabled)),
      Content::ReadOnly => Some(PropValue::Bool(self.read_only)),
      Content::Secure => Some(PropValue::Bool(self.secure)),
      Content::Keyboard => text(&self.keyboard),
      Content::Source => text(&self.source),
      Content::Destination => text(&self.destination),
      Content::Pixels => self.pixels.clone().map(PropValue::Pixels),
      Content::Door(_) => None,
    }
  }

  /// The text a control's paragraph shows, and whether it is its placeholder.
  fn label(&self, kind: ControlKind) -> Option<(Rc<str>, bool)> {
    match kind {
      ControlKind::Button | ControlKind::Checkbox | ControlKind::Radio => {
        self.caption.clone().map(|caption| (caption, false))
      }
      ControlKind::Picker => self.choice.as_ref().map(|choice| {
        let chosen = choice
          .chosen
          .and_then(|index| choice.options.get(index as usize).cloned())
          .unwrap_or_else(|| Rc::from(""));

        (chosen, false)
      }),
      ControlKind::TextField | ControlKind::TextEditor => self
        .placeholder
        .clone()
        .map(|placeholder| (placeholder, true)),
      ControlKind::Stepper => self
        .range
        .map(|range| (Rc::from(format_number(range.value, range.step)), false)),
      ControlKind::Switch | ControlKind::Slider | ControlKind::Progress => None,
    }
  }
}

/// A number shown with as many decimals as `step` needs.
fn format_number(value: f64, step: f64) -> String {
  let decimals = if step.fract() == 0. {
    0
  } else {
    (-step.abs().log10().floor()).clamp(0., 6.) as usize
  };

  format!("{value:.decimals$}")
}

/// The space around a control's label.
const PADDING: (f32, f32) = (14., 7.);
/// The least height of a control drawn as a box.
const HEIGHT: f32 = 34.;
/// A switch's track.
const SWITCH: (f32, f32) = (44., 24.);
/// A checkbox's or a radio's mark.
const MARK: f32 = 18.;
/// The space between a mark and its caption.
const GAP: f32 = 8.;
/// A slider's or a progress's natural width.
const BAR: f32 = 200.;

impl Drawn {
  /// The paragraph a control shows, built from its contents and the run properties it inherits,
  /// or `None` where it shows none.
  pub fn control_label(
    self: &Rc<Self>,
    kind: ControlKind,
    inherited: &RunProps,
    palette: &Palette,
  ) -> Option<(AttributedText, skene_foundation::text::ParagraphStyle)> {
    let contents = self.contents.borrow();
    let (text, placeholder) = contents.label(kind)?;
    let ink = if placeholder || contents.disabled {
      palette.dim
    } else if kind == ControlKind::Button && contents.prominent {
      palette.on_accent
    } else {
      palette.ink
    };
    let mut props = inherited.clone();

    if placeholder || (kind == ControlKind::Button && contents.prominent) {
      props.color = None;
    }

    let style = props.run_style(ink, DEFAULT_SIZE);
    let mut paragraph = props.paragraph_style();

    if kind != ControlKind::TextEditor {
      paragraph = paragraph.with_wrap(skene_foundation::text::Wrap::None);
    }

    Some((AttributedText::plain(text, style), paragraph))
  }

  fn caption_cell(self: &Rc<Self>) -> Option<Rc<TextCell>> {
    self.text.borrow().clone()
  }

  /// The control's size along `axis`, given the other axis's size where known.
  pub fn measure_control(
    self: &Rc<Self>,
    kind: ControlKind,
    axis: Axis,
    for_size: Option<f32>,
  ) -> Measurement {
    let text = self.caption_cell();
    let intrinsic = text
      .as_ref()
      .map(|cell| cell.intrinsic())
      .unwrap_or_default();
    let laid = text.as_ref().and_then(|cell| cell.laid(None));
    let (text_height, baseline) = laid.as_ref().map_or((17., 13.), |laid| {
      (laid.size().height, laid.first_baseline())
    });
    let fixed = |width: f32, height: f32, baseline: Option<f32>| match axis {
      Axis::Horizontal => Measurement {
        minimum: width,
        natural: width,
        baseline: None,
      },
      Axis::Vertical => Measurement {
        minimum: height,
        natural: height,
        baseline,
      },
    };
    let boxed_height = (text_height + PADDING.1 * 2.).max(HEIGHT);
    let boxed_baseline = Some((boxed_height - text_height) / 2. + baseline);

    match kind {
      ControlKind::Button => fixed(
        (intrinsic.max + PADDING.0 * 2.).ceil().max(HEIGHT),
        boxed_height,
        boxed_baseline,
      ),
      ControlKind::Picker => {
        let widest = self
          .contents
          .borrow()
          .choice
          .as_ref()
          .zip(text.as_ref())
          .map_or(0., |(choice, cell)| {
            choice
              .options
              .iter()
              .map(|option| cell.intrinsic_of(option).max)
              .fold(0., f32::max)
          });

        fixed(
          (intrinsic.max.max(widest) + PADDING.0 * 2. + 20.).ceil(),
          boxed_height,
          boxed_baseline,
        )
      }
      ControlKind::Switch => fixed(SWITCH.0, SWITCH.1, None),
      ControlKind::Checkbox | ControlKind::Radio => {
        let width = if text.is_some() {
          MARK + GAP + intrinsic.max
        } else {
          MARK
        };
        let height = text_height.max(MARK);

        fixed(
          width.ceil(),
          height.ceil(),
          Some((height - text_height) / 2. + baseline),
        )
      }
      ControlKind::Slider => match axis {
        Axis::Horizontal => Measurement {
          minimum: 80.,
          natural: BAR,
          baseline: None,
        },
        Axis::Vertical => fixed(0., 24., None),
      },
      ControlKind::Progress => match axis {
        Axis::Horizontal => Measurement {
          minimum: 40.,
          natural: BAR,
          baseline: None,
        },
        Axis::Vertical => fixed(0., 8., None),
      },
      ControlKind::TextField => {
        let (line, baseline) = self.entry_line_metrics();
        let height = (line + PADDING.1 * 2.).max(HEIGHT);

        match axis {
          Axis::Horizontal => Measurement {
            minimum: 60.,
            natural: 200.,
            baseline: None,
          },
          Axis::Vertical => fixed(0., height, Some((height - line) / 2. + baseline)),
        }
      }
      ControlKind::Stepper => match axis {
        Axis::Horizontal => Measurement {
          minimum: 60.,
          natural: 120.,
          baseline: None,
        },
        Axis::Vertical => fixed(0., boxed_height, boxed_baseline),
      },
      ControlKind::TextEditor => match axis {
        Axis::Horizontal => Measurement {
          minimum: 60.,
          natural: 200.,
          baseline: None,
        },
        Axis::Vertical => {
          let (_, baseline) = self.entry_line_metrics();
          let height = self.entry_height_at(for_size).max(HEIGHT);

          fixed(0., height, Some(crate::entry::INSET.1 + baseline))
        }
      },
    }
  }

  fn is_hovered(self: &Rc<Self>) -> bool {
    self.states.get().contains(State::Hovered)
  }

  fn is_pressed(self: &Rc<Self>) -> bool {
    self.states.get().contains(State::Pressed)
  }

  /// Draws the control in `rect` (window pixels).
  pub fn paint_control(
    self: &Rc<Self>,
    kind: ControlKind,
    canvas: &mut Canvas<'_>,
    rect: Rect,
    marks: &Marks,
  ) {
    let palette = canvas.palette;
    let contents = self.contents.borrow().clone();
    let laid = self.caption_cell().and_then(|cell| cell.laid(None));
    let focused = marks
      .focus
      .as_ref()
      .is_some_and(|focus| Rc::ptr_eq(focus, self));
    let own = self.look.borrow().clone();
    let fill_for = |base: Color| {
      if contents.disabled {
        base
      } else if self.is_pressed() {
        palette.control_pressed
      } else if self.is_hovered() {
        palette.control_hover
      } else {
        base
      }
    };
    let center_text = |canvas: &mut Canvas<'_>, left: f32| {
      if let Some(laid) = &laid {
        let size = laid.size();

        canvas.text(laid, (left, rect.y + (rect.height - size.height) / 2.));
      }
    };
    let corners = if own.corners == Corners::all(0.) {
      Corners::all(8.)
    } else {
      own.corners
    };

    match kind {
      ControlKind::Button => {
        if own.background.is_none() {
          let fill = if contents.prominent {
            let accent = palette.accent;

            if self.is_pressed() {
              mix(accent, Color::BLACK, 0.2)
            } else if self.is_hovered() {
              mix(accent, Color::BLACK, 0.1)
            } else {
              accent
            }
          } else {
            fill_for(palette.control)
          };

          canvas.fill(rect, corners, fill);
        }

        if let Some(laid) = &laid {
          let width = laid.size().width;

          center_text(canvas, rect.x + (rect.width - width) / 2.);
        }

        if focused {
          focus_ring(canvas, rect, corners);
        }
      }
      ControlKind::Picker => {
        canvas.fill(rect, corners, fill_for(palette.control));
        center_text(canvas, rect.x + PADDING.0);

        let (x, y) = (
          rect.x + rect.width - PADDING.0 - 6.,
          rect.y + rect.height / 2.,
        );
        let mut chevron = BezPath::new();

        chevron.move_to((f64::from(x - 5.), f64::from(y - 2.5)));
        chevron.line_to((f64::from(x), f64::from(y + 2.5)));
        chevron.line_to((f64::from(x + 5.), f64::from(y - 2.5)));
        canvas.stroke_path(&chevron, 1.5, palette.ink);

        if focused {
          focus_ring(canvas, rect, corners);
        }
      }
      ControlKind::Switch => {
        let track = Rect {
          x: rect.x + (rect.width - SWITCH.0) / 2.,
          y: rect.y + (rect.height - SWITCH.1) / 2.,
          width: SWITCH.0,
          height: SWITCH.1,
        };
        let fill = if contents.on {
          palette.accent
        } else {
          fill_for(palette.track)
        };
        let knob_x = if contents.on {
          track.x + track.width - track.height / 2.
        } else {
          track.x + track.height / 2.
        };

        canvas.fill(track, Corners::all(track.height / 2.), fill);
        canvas.circle(
          (knob_x, track.y + track.height / 2.),
          track.height / 2. - 2.,
          palette.knob,
        );

        if focused {
          focus_ring(canvas, track, Corners::all(track.height / 2.));
        }
      }
      ControlKind::Checkbox | ControlKind::Radio => {
        let mark = Rect {
          x: rect.x,
          y: rect.y + (rect.height - MARK) / 2.,
          width: MARK,
          height: MARK,
        };
        let round = if kind == ControlKind::Radio {
          Corners::all(MARK / 2.)
        } else {
          Corners::all(4.)
        };

        if contents.on || contents.mixed {
          canvas.fill(mark, round, palette.accent);

          if kind == ControlKind::Radio {
            canvas.circle(
              (mark.x + MARK / 2., mark.y + MARK / 2.),
              MARK / 5.,
              palette.on_accent,
            );
          } else if contents.mixed {
            canvas.fill(
              Rect {
                x: mark.x + 4.,
                y: mark.y + MARK / 2. - 1.,
                width: MARK - 8.,
                height: 2.,
              },
              Corners::all(0.),
              palette.on_accent,
            );
          } else {
            let mut tick = BezPath::new();

            tick.move_to((f64::from(mark.x + 4.5), f64::from(mark.y + 9.5)));
            tick.line_to((f64::from(mark.x + 7.5), f64::from(mark.y + 12.5)));
            tick.line_to((f64::from(mark.x + 13.5), f64::from(mark.y + 5.5)));
            canvas.stroke_path(&tick, 2., palette.on_accent);
          }
        } else {
          canvas.fill(mark, round, fill_for(palette.field));
          canvas.border(mark, round, 1.5, BorderStyle::Solid, palette.outline);
        }

        center_text(canvas, rect.x + MARK + GAP);

        if focused {
          focus_ring(canvas, mark, round);
        }
      }
      ControlKind::Slider | ControlKind::Progress => {
        let range = contents.range.unwrap_or(Range {
          value: 0.,
          minimum: 0.,
          maximum: 1.,
          step: 0.01,
        });
        let span = (range.maximum - range.minimum).max(f64::EPSILON);
        let fraction = ((range.value - range.minimum) / span).clamp(0., 1.) as f32;
        let (inset, thickness) = if kind == ControlKind::Slider {
          (10., 4.)
        } else {
          (0., 6.)
        };
        let track = Rect {
          x: rect.x + inset,
          y: rect.y + (rect.height - thickness) / 2.,
          width: (rect.width - inset * 2.).max(0.),
          height: thickness,
        };
        let filled = Rect {
          width: track.width * fraction,
          ..track
        };

        canvas.fill(track, Corners::all(thickness / 2.), palette.track);
        canvas.fill(filled, Corners::all(thickness / 2.), palette.accent);

        if kind == ControlKind::Slider {
          let center = (track.x + track.width * fraction, rect.y + rect.height / 2.);
          let knob = if self.is_pressed() { 10. } else { 9. };

          canvas.circle(
            (center.0, center.1 + 0.5),
            knob + 0.5,
            mix(palette.outline, palette.window, 0.3),
          );
          canvas.circle(center, knob, palette.knob);

          if focused {
            focus_ring(
              canvas,
              Rect {
                x: center.0 - knob,
                y: center.1 - knob,
                width: knob * 2.,
                height: knob * 2.,
              },
              Corners::all(knob),
            );
          }
        }
      }
      ControlKind::TextField | ControlKind::TextEditor | ControlKind::Stepper => {
        let fill = own.background.unwrap_or(palette.field);

        if own.background.is_none() {
          canvas.fill(rect, corners, fill);
        }

        if own.border.is_none() {
          let (width, ink) = if focused {
            (2., palette.accent)
          } else {
            (1., palette.outline)
          };

          canvas.border(rect, corners, width, BorderStyle::Solid, ink);
        }

        if is_entry(kind) {
          self.paint_entry(canvas, rect, focused);
        } else if laid.is_some() {
          center_text(canvas, rect.x + 10.);
        }

        if kind == ControlKind::Stepper {
          let half = rect.height / 2.;
          let x = rect.x + rect.width - 24.;

          for (row, up) in [(0., true), (half, false)] {
            let y = rect.y + row + half / 2.;
            let mut arrow = BezPath::new();
            let (tip, base) = if up {
              (y - 2.5, y + 2.5)
            } else {
              (y + 2.5, y - 2.5)
            };

            arrow.move_to((f64::from(x + 7.), f64::from(base)));
            arrow.line_to((f64::from(x + 12.), f64::from(tip)));
            arrow.line_to((f64::from(x + 17.), f64::from(base)));
            canvas.stroke_path(&arrow, 1.5, palette.ink);
          }
        }
      }
    }
  }

  /// Draws the bar a scrolled box shows where its content overflows it.
  pub fn paint_scroll_bar(self: &Rc<Self>, canvas: &mut Canvas<'_>, rect: Rect) {
    let (_, extent) = self.extent.get();
    let (_, offset) = self.scroll.get();

    if extent <= rect.height + 0.5 {
      return;
    }

    let length = (rect.height * rect.height / extent).max(24.);
    let travel = rect.height - length;
    let at = travel * (offset / (extent - rect.height)).clamp(0., 1.);
    let ink = canvas
      .palette
      .ink
      .with_alpha(if self.is_hovered() { 110 } else { 70 });

    canvas.fill(
      Rect {
        x: rect.x + rect.width - 7.,
        y: rect.y + at + 2.,
        width: 4.,
        height: length - 4.,
      },
      Corners::all(2.),
      ink,
    );
  }

  /// The on/off control here as its behavior sees it.
  pub fn toggle_behavior(self: &Rc<Self>, kind: ToggleKind) -> Toggle {
    let contents = self.contents.borrow();

    Toggle::new(kind, contents.on, contents.mixed, self.key_pressed.get())
  }

  /// The range a slider or a stepper here shows, its default where none was written.
  pub fn range(self: &Rc<Self>) -> Range {
    self.contents.borrow().range.unwrap_or(Range {
      value: 0.,
      minimum: 0.,
      maximum: 1.,
      step: 0.01,
    })
  }

  /// The picker here as its behavior sees it: open with `highlighted` where `open`.
  pub fn popup_behavior(self: &Rc<Self>, open: Option<Option<usize>>) -> Popup {
    let contents = self.contents.borrow();
    let (count, chosen) = contents.choice.as_ref().map_or((0, None), |choice| {
      (
        choice.options.len(),
        choice.chosen.map(|index| index as usize),
      )
    });

    Popup::new(count, chosen).with_open(open.is_some(), open.flatten())
  }

  /// A choice of option `index` of its picker.
  pub fn choice_change(self: &Rc<Self>, index: usize) -> Option<Act> {
    let choice = self.contents.borrow().choice.clone()?;
    let index = u32::try_from(index).ok()?;

    (choice.chosen != Some(index)).then_some(Act::Change(
      Content::Choice,
      PropValue::Choice(Choice {
        options: choice.options,
        chosen: Some(index),
      }),
    ))
  }

  /// What a primary press at `x`, `y` (window pixels) in a control at `rect` does at once: a
  /// slider's thumb jumps there.
  pub fn press_control(
    self: &Rc<Self>,
    kind: ControlKind,
    rect: Rect,
    x: f32,
    _: f32,
  ) -> Option<Act> {
    if self.contents.borrow().disabled {
      return None;
    }

    match kind {
      ControlKind::Slider => {
        let range = self.range();
        let (left, width) = track(rect);

        Slider::new(range)
          .at(f64::from((x - left) / width))
          .map(|value| range_change(range, value))
      }
      _ => None,
    }
  }

  /// What a drag to `x`, `y` (window pixels) does after a press in the control.
  pub fn drag_control(
    self: &Rc<Self>,
    kind: ControlKind,
    rect: Rect,
    x: f32,
    y: f32,
  ) -> Option<Act> {
    self.press_control(kind, rect, x, y)
  }

  /// What a click (a press and a release over the control) at `x`, `y` does.
  pub fn click_control(
    self: &Rc<Self>,
    kind: ControlKind,
    rect: Rect,
    _: f32,
    y: f32,
    open: Option<Option<usize>>,
  ) -> Option<Act> {
    if self.contents.borrow().disabled {
      return None;
    }

    if let Some(toggle_kind) = toggle_kind(kind) {
      return self
        .toggle_behavior(toggle_kind)
        .click()
        .map(|on| Act::Change(Content::On, PropValue::Bool(on)));
    }

    match kind {
      ControlKind::Button => Activate::new(false).click().map(|()| Act::Activate),
      ControlKind::Picker => self.popup_behavior(open).click().map(Act::Popup),
      ControlKind::Stepper => {
        let range = self.range();
        let up = y < rect.y + rect.height / 2.;
        let slider = Slider::new(range);
        let step = range.step.max(f64::EPSILON);

        slider
          .moved(range.value + if up { step } else { -step })
          .map(|value| range_change(range, value))
      }
      _ => None,
    }
  }

  /// What a key does to the focused control (its picker open with a highlight where `open`).
  pub fn key_control(
    self: &Rc<Self>,
    kind: ControlKind,
    event: &KeyEvent,
    open: Option<Option<usize>>,
  ) -> KeyResult {
    if self.contents.borrow().disabled {
      return KeyResult::IGNORED;
    }

    if let Some(toggle_kind) = toggle_kind(kind) {
      return KeyResult::from(self.toggle_behavior(toggle_kind).key(event), |on| {
        Act::Change(Content::On, PropValue::Bool(on))
      });
    }

    match kind {
      ControlKind::Button => {
        KeyResult::from(Activate::new(self.key_pressed.get()).key(event), |()| {
          Act::Activate
        })
      }
      ControlKind::Slider | ControlKind::Stepper => {
        let range = self.range();

        KeyResult::from(Slider::new(range).key(event), |value| {
          range_change(range, value)
        })
      }
      ControlKind::Picker => KeyResult::from(self.popup_behavior(open).key(event), Act::Popup),
      _ => KeyResult::IGNORED,
    }
  }

  /// What a reader's action does to a control.
  pub fn control_action(
    self: &Rc<Self>,
    kind: ControlKind,
    action: Action,
    open: Option<Option<usize>>,
  ) -> Option<Act> {
    if self.contents.borrow().disabled {
      return None;
    }

    if let Some(toggle_kind) = toggle_kind(kind) {
      return self
        .toggle_behavior(toggle_kind)
        .action(action)
        .map(|on| Act::Change(Content::On, PropValue::Bool(on)));
    }

    match kind {
      ControlKind::Button => Activate::new(false).action(action).map(|()| Act::Activate),
      ControlKind::Slider | ControlKind::Stepper => {
        let range = self.range();

        Slider::new(range)
          .action(action)
          .map(|value| range_change(range, value))
      }
      ControlKind::Picker => self.popup_behavior(open).action(action).map(Act::Popup),
      _ => None,
    }
  }

  /// What a control tells a reader of its state (its check, its value, its expansion) and the
  /// actions it offers.
  pub fn control_facts(
    self: &Rc<Self>,
    kind: ControlKind,
    open: Option<Option<usize>>,
  ) -> AccessFacts {
    let behaved = match toggle_kind(kind) {
      Some(toggle_kind) => {
        let toggle = self.toggle_behavior(toggle_kind);

        AccessFacts {
          actions: toggle.actions(),
          ..toggle.facts()
        }
      }
      None => match kind {
        ControlKind::Button => AccessFacts {
          actions: Activate::new(false).actions(),
          ..AccessFacts::default()
        },
        ControlKind::Slider | ControlKind::Stepper | ControlKind::Progress => {
          let slider = Slider::new(self.range());

          AccessFacts {
            actions: if kind == ControlKind::Progress {
              Vec::new()
            } else {
              slider.actions()
            },
            ..slider.facts()
          }
        }
        ControlKind::Picker => {
          let popup = self.popup_behavior(open);

          AccessFacts {
            actions: popup.actions(),
            ..popup.facts()
          }
        }
        ControlKind::TextField | ControlKind::TextEditor => AccessFacts {
          read_only: self.contents.borrow().read_only,
          ..AccessFacts::default()
        },
        ControlKind::Switch | ControlKind::Checkbox | ControlKind::Radio => AccessFacts::default(),
      },
    };

    AccessFacts {
      focusable: focusable(kind),
      ..behaved
    }
  }
}

/// Whether a control of `kind` takes focus.
pub const fn focusable(kind: ControlKind) -> bool {
  !matches!(kind, ControlKind::Progress)
}

/// `a` moved toward `b` by `amount`.
pub fn mix(a: Color, b: Color, amount: f32) -> Color {
  let channel = |a: u8, b: u8| -> u8 {
    let value = f32::from(a) + (f32::from(b) - f32::from(a)) * amount;

    value.round().clamp(0., 255.) as u8
  };

  Color {
    red: channel(a.red, b.red),
    green: channel(a.green, b.green),
    blue: channel(a.blue, b.blue),
    alpha: channel(a.alpha, b.alpha),
  }
}

/// The on/off behavior of a control of `kind`, where it is one.
pub const fn toggle_kind(kind: ControlKind) -> Option<ToggleKind> {
  match kind {
    ControlKind::Switch => Some(ToggleKind::Switch),
    ControlKind::Checkbox => Some(ToggleKind::Checkbox),
    ControlKind::Radio => Some(ToggleKind::Radio),
    _ => None,
  }
}

/// What the reader did to a control, to be reported or carried out.
#[derive(Clone, PartialEq, Debug)]
pub enum Act {
  /// Activated it.
  Activate,
  /// Changed `content` to the value.
  Change(Content, PropValue),
  /// Opened, closed or moved in its list.
  Popup(PopupAct),
}

/// What a key did to a control.
#[derive(Clone, PartialEq, Debug)]
pub struct KeyResult {
  /// Whether the control consumed it.
  pub consumed: bool,
  /// Whether it now shows pressed from the keyboard, where that changed.
  pub pressed: Option<bool>,
  /// What it reports.
  pub act: Option<Act>,
}

impl KeyResult {
  fn from<A>(response: Response<A>, map: impl FnOnce(A) -> Act) -> Self {
    Self {
      consumed: response.consumed,
      pressed: response.pressed,
      act: response.act.map(map),
    }
  }

  const IGNORED: Self = Self {
    consumed: false,
    pressed: None,
    act: None,
  };
}

/// A proposal of `value` for `range`'s value.
fn range_change(range: Range, value: f64) -> Act {
  Act::Change(Content::Range, PropValue::Range(Range { value, ..range }))
}

/// Where a slider's track runs in a control at `rect` (window pixels): its left and its
/// width.
fn track(rect: Rect) -> (f32, f32) {
  (rect.x + 10., (rect.width - 20.).max(1.))
}

/// Whether a control of `kind` is a text entry, where the pointer shows a text cursor.
pub const fn is_entry(kind: ControlKind) -> bool {
  matches!(kind, ControlKind::TextField | ControlKind::TextEditor)
}
