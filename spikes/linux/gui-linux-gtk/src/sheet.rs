//! The style sheet: one display-wide provider of content-addressed classes at priority 599,
//! reference-counted and collected; one of named colors at 598, the tokens' definitions, each
//! named for its content tag; and a per-widget provider for derived properties. An app's own
//! CSS at the application priority (600) beats Skene's.

use std::{
  cell::{Cell, RefCell},
  collections::BTreeMap,
};

use gtk::{glib, prelude::*};
use skene_foundation::{
  Color, State, StateSet,
  seam::{
    record::{Prop, Resolved},
    style::{CompiledStyle, KeptDefinition, KeptLeaf, KeptValue, ScopeId},
  },
  value::{
    BorderStyle, Corners, Curve, FontFamily, FontStyle, Shadows, TextAlignment, TextDecorationStyle,
  },
};

use crate::{SkeneButton, children, slot};

/// What the sheet has done since the thread started: a test reads it to prove what a change
/// cost.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct SheetStats {
  /// Styles applied to nodes.
  pub applies: u32,
  /// Reloads of the class sheet.
  pub reloads: u32,
  /// Rules collected, no widget referring to them.
  pub collected: u32,
  /// Loads of a per-widget provider of derived properties.
  pub slot_writes: u32,
  /// Reloads of the token sheet.
  pub token_reloads: u32,
}

thread_local! {
  static STATS: Cell<SheetStats> = const {
    Cell::new(SheetStats {
      applies: 0,
      reloads: 0,
      collected: 0,
      slot_writes: 0,
      token_reloads: 0,
    })
  };
}

fn count(change: impl FnOnce(&mut SheetStats)) {
  STATS.with(|stats| {
    let mut now = stats.get();

    change(&mut now);
    stats.set(now);
  });
}

/// What the sheet has done since the thread started.
#[must_use]
pub fn stats() -> SheetStats {
  STATS.with(Cell::get)
}

struct Rule {
  css: String,
  references: usize,
}

struct Sheet {
  provider: gtk::CssProvider,
  tokens: gtk::CssProvider,
  rules: BTreeMap<String, Rule>,
  colors: BTreeMap<ScopeId, String>,
  dirty: bool,
  swaps: Vec<(gtk::Widget, Option<String>)>,
  scheduled: bool,
}

thread_local! {
  static SHEET: RefCell<Option<Sheet>> = const { RefCell::new(None) };
}

/// What every Skene widget starts from, just above the theme: a box that acts shows the
/// keyboard's focus as the theme's controls do, and a view's own style overrides it.
const BASE: &str = "skenebutton { outline: 0 solid transparent; outline-offset: -2px; }
skenebutton:focus-visible { outline: 2px solid alpha(currentColor, 0.45); }";

/// Installs the sheet's providers on the display, once per thread.
pub(crate) fn install() {
  with_sheet(|_| ());
}

fn with_sheet<T>(body: impl FnOnce(&mut Sheet) -> T) -> T {
  SHEET.with(|sheet| {
    let mut sheet = sheet.borrow_mut();
    let sheet = sheet.get_or_insert_with(|| {
      let provider = gtk::CssProvider::new();
      let tokens = gtk::CssProvider::new();
      let base = gtk::CssProvider::new();
      let display = gtk::gdk::Display::default().expect("a display");

      base.load_from_string(BASE);

      gtk::style_context_add_provider_for_display(
        &display,
        &base,
        gtk::STYLE_PROVIDER_PRIORITY_THEME + 1,
      );
      gtk::style_context_add_provider_for_display(
        &display,
        &tokens,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 2,
      );
      gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 1,
      );

      Sheet {
        provider,
        tokens,
        rules: BTreeMap::new(),
        colors: BTreeMap::new(),
        dirty: false,
        swaps: Vec::new(),
        scheduled: false,
      }
    });

    body(sheet)
  })
}

/// Reloads the sheet where a rule is new, then swaps the classes, in one dispatch above the
/// redraw's priority, so a rule and its class land in the same frame.
fn schedule_flush() {
  let schedule = with_sheet(|sheet| !std::mem::replace(&mut sheet.scheduled, true));

  if schedule {
    glib::idle_add_local_full(glib::Priority::HIGH_IDLE, || {
      flush();

      glib::ControlFlow::Break
    });
  }
}

/// How many rule groups the sheet holds.
pub fn rule_count() -> usize {
  with_sheet(|sheet| sheet.rules.len())
}

/// How many theme scopes define named colors now.
pub fn token_scopes() -> usize {
  with_sheet(|sheet| sheet.colors.len())
}

/// Moves `widget`'s reference from the class it held to `class`.
fn retarget(widget: &gtk::Widget, class: Option<String>) {
  let old = slot(widget).target.replace(class.clone());

  if old == class {
    return;
  }

  with_sheet(|sheet| {
    if let Some(rule) = old.and_then(|old| sheet.rules.get_mut(&old)) {
      rule.references = rule.references.saturating_sub(1);
    }

    if let Some(rule) = class.as_ref().and_then(|class| sheet.rules.get_mut(class)) {
      rule.references += 1;
    }

    sheet.swaps.push((widget.clone(), class));
  });
  schedule_flush();
}

/// Applies what the sheet holds now: rules no widget refers to are collected when the sheet
/// reloads anyway, for a new rule.
pub fn flush() {
  let (reload, swaps) = with_sheet(|sheet| {
    sheet.scheduled = false;

    let reload = std::mem::take(&mut sheet.dirty).then(|| {
      let before = sheet.rules.len();

      sheet.rules.retain(|_, rule| rule.references > 0);
      count(|stats| stats.collected += (before - sheet.rules.len()) as u32);

      sheet
        .rules
        .values()
        .map(|rule| rule.css.as_str())
        .collect::<String>()
    });

    (
      reload.map(|css| (sheet.provider.clone(), css)),
      std::mem::take(&mut sheet.swaps),
    )
  });

  if let Some((provider, css)) = reload {
    count(|stats| stats.reloads += 1);
    provider.load_from_string(&css);
  }

  for (widget, class) in swaps {
    let slot = slot(&widget);
    let old = slot.class.replace(class.clone());

    if let Some(old) = old {
      widget.remove_css_class(&old);
    }

    if let Some(class) = class {
      widget.add_css_class(&class);
    }
  }
}

fn token_name(name: &str, tag: u64) -> String {
  let name: String = name
    .chars()
    .map(|character| {
      if character.is_ascii_alphanumeric() {
        character
      } else {
        '_'
      }
    })
    .collect();

  format!("skene_{name}_{tag:x}")
}

fn css_color(color: Color) -> String {
  format!(
    "rgba({}, {}, {}, {:.3})",
    color.red,
    color.green,
    color.blue,
    f32::from(color.alpha) / 255.
  )
}

fn css_leaf(leaf: &KeptLeaf) -> Option<String> {
  match leaf {
    KeptLeaf::Token { name, tag } => Some(format!("@{}", token_name(name, *tag))),
    KeptLeaf::Value(Resolved::Color(color)) => Some(css_color(*color)),
    KeptLeaf::Value(Resolved::Number(number)) => Some(format!("{number}px")),
    KeptLeaf::Value(_) | KeptLeaf::Slot(_) => None,
  }
}

/// `corners` as CSS's `border-radius`, the leading corners on the left, or on the right where
/// `rtl`.
fn css_corners(corners: &Corners, rtl: bool) -> String {
  let (top_left, top_right, bottom_right, bottom_left) = if rtl {
    (
      corners.top_trailing,
      corners.top_leading,
      corners.bottom_leading,
      corners.bottom_trailing,
    )
  } else {
    (
      corners.top_leading,
      corners.top_trailing,
      corners.bottom_trailing,
      corners.bottom_leading,
    )
  };

  format!("border-radius: {top_left}px {top_right}px {bottom_right}px {bottom_left}px;")
}

/// `shadows` as CSS's `box-shadow`, a trailing offset to the right, or to the left where
/// `rtl`.
fn css_shadows(shadows: &Shadows, rtl: bool) -> String {
  let list: Vec<String> = shadows
    .0
    .iter()
    .rev()
    .map(|shadow| {
      format!(
        "{}{}px {}px {}px {}px {}",
        if shadow.inset { "inset " } else { "" },
        if rtl { -shadow.x } else { shadow.x },
        shadow.y,
        shadow.blur,
        shadow.spread,
        css_color(shadow.color)
      )
    })
    .collect();

  format!("box-shadow: {};", list.join(", "))
}

/// The declarations of one property that differ under right to left: its leading and trailing
/// sides swapped.
fn mirrored(prop: Prop, leaf: &KeptLeaf) -> Option<String> {
  match (prop, leaf) {
    (Prop::CornerRadius, KeptLeaf::Value(Resolved::Corners(corners))) => {
      Some(css_corners(corners, true))
    }
    (Prop::Shadow, KeptLeaf::Value(Resolved::Shadows(shadows))) => Some(css_shadows(shadows, true)),
    _ => None,
  }
}

/// The family GTK draws its interface in (`gtk-font-name`), `system-ui` where it names none.
fn interface_family() -> String {
  gtk::Settings::default()
    .and_then(|settings| settings.gtk_font_name())
    .and_then(|name| gtk::pango::FontDescription::from_string(&name).family())
    .map_or_else(|| "system-ui".to_owned(), |family| format!("\"{family}\""))
}

/// The GTK CSS declarations of one property, or none where GTK 4.14 CSS cannot express it
/// (those are applied through the widget's API, or not at all).
fn declarations(prop: Prop, leaf: &KeptLeaf) -> Option<String> {
  use Resolved as R;

  let value = match leaf {
    KeptLeaf::Value(value) => Some(value),
    _ => None,
  };

  Some(match (prop, value) {
    (Prop::Background, _) => format!(
      "background-color: {}; background-image: none;",
      css_leaf(leaf)?
    ),
    (Prop::Color, _) => format!("color: {};", css_leaf(leaf)?),
    (Prop::DecorationColor, _) => format!("text-decoration-color: {};", css_leaf(leaf)?),
    (Prop::DecorationStyle, Some(R::TextDecorationStyle(style))) => format!(
      "text-decoration-style: {};",
      match style {
        TextDecorationStyle::Double => "double",
        TextDecorationStyle::Wavy => "wavy",
        TextDecorationStyle::Solid | TextDecorationStyle::Dotted | TextDecorationStyle::Dashed => {
          "solid"
        }
      }
    ),
    (Prop::Border, Some(R::Border(border))) => format!(
      "border-width: {}px; border-style: {};",
      border.width,
      match border.style {
        BorderStyle::Solid => "solid",
        BorderStyle::Dashed => "dashed",
        BorderStyle::Dotted => "dotted",
        BorderStyle::Double => "double",
      }
    ),
    (Prop::BorderColor, _) => format!("border-color: {};", css_leaf(leaf)?),
    (Prop::CornerRadius, Some(R::Corners(corners))) => css_corners(corners, false),
    (Prop::Shadow, Some(R::Shadows(shadows))) => css_shadows(shadows, false),
    (Prop::Opacity, Some(R::Number(opacity))) => format!("opacity: {opacity};"),
    (Prop::FontSize, Some(R::Number(size))) => format!("font-size: {size}px;"),
    (Prop::FontWeight, Some(R::FontWeight(weight))) => format!("font-weight: {};", weight.0),
    (Prop::FontStyle, Some(R::FontStyle(style))) => format!(
      "font-style: {};",
      match style {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
      }
    ),
    (Prop::FontFamily, Some(R::FontFamily(family))) => format!(
      "font-family: {};",
      match family {
        FontFamily::System => interface_family(),
        FontFamily::Monospace => "monospace".to_owned(),
        FontFamily::Named(name) => format!("\"{name}\""),
      }
    ),
    (Prop::LineHeight, Some(R::Number(height))) => format!("line-height: {height};"),
    (Prop::LetterSpacing, Some(R::Number(spacing))) => format!("letter-spacing: {spacing}px;"),
    (Prop::Decoration, Some(R::TextDecoration(decoration))) => {
      let mut lines = Vec::new();

      if decoration.underline {
        lines.push("underline");
      }

      if decoration.line_through {
        lines.push("line-through");
      }

      if decoration.overline {
        lines.push("overline");
      }

      if lines.is_empty() {
        lines.push("none");
      }

      format!("text-decoration-line: {};", lines.join(" "))
    }
    _ => return None,
  })
}

const fn css_property(prop: Prop) -> Option<&'static str> {
  Some(match prop {
    Prop::Background => "background-color",
    Prop::Color => "color",
    Prop::Border => "border-width",
    Prop::BorderColor => "border-color",
    Prop::CornerRadius => "border-radius",
    Prop::Shadow => "box-shadow",
    Prop::Opacity => "opacity",
    Prop::FontSize => "font-size",
    Prop::FontWeight => "font-weight",
    Prop::LineHeight => "line-height",
    Prop::LetterSpacing => "letter-spacing",
    Prop::DecorationColor => "text-decoration-color",
    _ => return None,
  })
}

fn css_curve(curve: Curve) -> String {
  match curve {
    Curve::Linear => "linear".to_owned(),
    Curve::EaseInOut => "ease-in-out".to_owned(),
    Curve::EaseIn => "ease-in".to_owned(),
    Curve::EaseOut => "ease-out".to_owned(),
    Curve::Bezier(x1, y1, x2, y2) => format!("cubic-bezier({x1}, {y1}, {x2}, {y2})"),
  }
}

const fn pseudo(state: State) -> &'static str {
  match state {
    State::Hovered => ":hover",
    State::Pressed => ":active",
    State::FocusVisible => ":focus-visible",
    State::FocusWithin => ":focus-within",
    State::Disabled => ":disabled",
    State::Checked | State::Open => ":checked",
    State::Indeterminate => ":indeterminate",
    State::Invalid => ".error",
    State::Selected => ":selected",
  }
}

/// The rules of `style` under `selector`, its slot leaves taken from `slots`; `head` opens the
/// base rule (a reset, the transitions).
fn rules(selector: &str, style: &CompiledStyle, head: &str) -> String {
  let mut css = String::new();

  for (states, props) in style.state_rules() {
    let selector: String = std::iter::once(selector.to_owned())
      .chain(states.iter().map(|state| pseudo(state).to_owned()))
      .collect();
    let mut body = if states.is_empty() {
      head.to_owned()
    } else {
      String::new()
    };
    let mut rtl = String::new();

    for (prop, leaf) in props {
      let leaf = match leaf {
        KeptLeaf::Slot(index) => style.slots.get(usize::from(*index)).unwrap_or(leaf),
        leaf => leaf,
      };

      if let Some(declaration) = declarations(prop, leaf) {
        body.push_str(&declaration);
        body.push(' ');
      }

      if let Some(declaration) = mirrored(prop, leaf) {
        rtl.push_str(&declaration);
        rtl.push(' ');
      }
    }

    css.push_str(&format!("{selector} {{ {body}}}\n"));

    if !rtl.is_empty() {
      css.push_str(&format!("{selector}:dir(rtl) {{ {rtl}}}\n"));
    }
  }

  css
}

/// `style` with only the properties `keep` accepts.
fn only(style: &CompiledStyle, keep: impl Fn(&KeptValue) -> bool) -> CompiledStyle {
  CompiledStyle {
    props: style
      .props
      .iter()
      .filter(|(_, value)| keep(value))
      .cloned()
      .collect(),
    ..style.clone()
  }
}

/// The base rule's head: the transitions.
fn head(style: &CompiledStyle) -> String {
  let mut head = String::new();
  let transitions: Vec<String> = style
    .transitions
    .iter()
    .filter_map(|transition| {
      Some(format!(
        "{} {}ms {} {}ms",
        css_property(transition.prop)?,
        transition.duration,
        css_curve(transition.curve),
        transition.delay
      ))
    })
    .collect();

  if !transitions.is_empty() {
    head.push_str(&format!("transition: {}; ", transitions.join(", ")));
  }

  head
}

/// Writes the derived properties of `widget`'s style into its own provider: a value that
/// changes costs this one widget's restyle, never a sheet reload, and adds no rule. The
/// properties GTK's CSS cannot express follow them through the widget's API.
pub fn write_slots(widget: &gtk::Widget, slots: &[KeptLeaf]) {
  let slot = slot(widget);
  let Some(style) = slot.style.borrow_mut().as_mut().map(|style| {
    style.slots = slots.to_vec();

    style.clone()
  }) else {
    return;
  };

  apply_widget_props(widget, &style);

  let mut derived = only(&style, KeptValue::has_slot);

  if derived.props.is_empty() && slot.provider.borrow().is_none() {
    return;
  }

  derived.slots = slots.to_vec();

  let css = rules("*", &derived, "");
  let provider = slot
    .provider
    .borrow_mut()
    .get_or_insert_with(|| {
      let provider = gtk::CssProvider::new();

      #[expect(deprecated, reason = "GTK 4.14's only per-widget style provider")]
      widget
        .style_context()
        .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION - 1);

      provider
    })
    .clone();

  count(|stats| stats.slot_writes += 1);
  provider.load_from_string(&css);
}

/// Applies `style` to `widget`: its class, made and referenced, and its derived properties;
/// the properties GTK's CSS cannot express (a box's clip, a label's alignment and lines) go
/// through the widget's API, with the leaf in force for its states and slots now, and follow
/// both.
pub fn apply(widget: &gtk::Widget, style: &CompiledStyle) {
  count(|stats| stats.applies += 1);

  let button = widget.is::<SkeneButton>();
  let class = format!("skene-{:x}", style.address);
  let shared = only(style, |value| !value.has_slot());

  with_sheet(|sheet| {
    if !sheet.rules.contains_key(&class) {
      let css = rules(&format!(".{class}"), &shared, &head(style));

      sheet
        .rules
        .insert(class.clone(), Rule { css, references: 0 });
      sheet.dirty = true;
    }
  });
  retarget(widget, Some(class));
  *slot(widget).style.borrow_mut() = Some(style.clone());
  write_slots(widget, &style.slots);

  if style.states().contains(State::Pressed) && !button {
    press_on_click(widget);
  }

  if WIDGET_PROPS.iter().any(|prop| {
    style
      .props
      .iter()
      .any(|(name, value)| name == prop && !value.variants.is_empty())
  }) {
    follow_states(widget);
  }
}

/// The properties GTK's CSS cannot express, applied through the widget's API.
const WIDGET_PROPS: [Prop; 3] = [Prop::Clip, Prop::TextAlignment, Prop::MaxLines];

/// The states `widget` is in now, as GTK's state flags and style classes say.
fn states_of(widget: &gtk::Widget) -> StateSet {
  let flags = widget.state_flags();

  [
    (gtk::StateFlags::PRELIGHT, State::Hovered),
    (gtk::StateFlags::ACTIVE, State::Pressed),
    (gtk::StateFlags::FOCUS_VISIBLE, State::FocusVisible),
    (gtk::StateFlags::FOCUS_WITHIN, State::FocusWithin),
    (gtk::StateFlags::INSENSITIVE, State::Disabled),
    (gtk::StateFlags::CHECKED, State::Checked),
    (gtk::StateFlags::CHECKED, State::Open),
    (gtk::StateFlags::INCONSISTENT, State::Indeterminate),
    (gtk::StateFlags::SELECTED, State::Selected),
  ]
  .into_iter()
  .filter(|(flag, _)| flags.contains(*flag))
  .map(|(_, state)| state)
  .chain(widget.has_css_class("error").then_some(State::Invalid))
  .collect()
}

/// Applies the properties of `style` GTK's CSS cannot express through `widget`'s API: each
/// the leaf in force for the widget's states now, a slot's value where it is derived.
fn apply_widget_props(widget: &gtk::Widget, style: &CompiledStyle) {
  let states = states_of(widget);
  let value = |prop: Prop| {
    let (_, kept) = style.props.iter().find(|(name, _)| *name == prop)?;

    match kept.with_states(states) {
      KeptLeaf::Slot(index) => match style.slots.get(usize::from(*index))? {
        KeptLeaf::Value(resolved) => Some(resolved.clone()),
        _ => None,
      },
      KeptLeaf::Value(resolved) => Some(resolved.clone()),
      KeptLeaf::Token { .. } => None,
    }
  };

  if let Some(Resolved::Flag(clip)) = value(Prop::Clip) {
    widget.set_overflow(if clip {
      gtk::Overflow::Hidden
    } else {
      gtk::Overflow::Visible
    });
  }

  if let Some(label) = widget.downcast_ref::<gtk::Label>() {
    if let Some(Resolved::TextAlignment(alignment)) = value(Prop::TextAlignment) {
      let (justify, xalign) = match alignment {
        TextAlignment::Leading => (gtk::Justification::Left, 0.),
        TextAlignment::Center => (gtk::Justification::Center, 0.5),
        TextAlignment::Trailing => (gtk::Justification::Right, 1.),
        TextAlignment::Justify => (gtk::Justification::Fill, 0.),
      };

      label.set_justify(justify);
      label.set_xalign(xalign);
    }

    if let Some(Resolved::Count(lines)) = value(Prop::MaxLines) {
      label.set_lines(lines as i32);
      label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    }
  }
}

/// Re-applies the properties GTK's CSS cannot express whenever `widget`'s states change, once.
fn follow_states(widget: &gtk::Widget) {
  const FOLLOWED: &str = "skene-follow-states";

  // SAFETY: the key only ever holds a `bool`, written here.
  if unsafe { widget.data::<bool>(FOLLOWED) }.is_some() {
    return;
  }

  // SAFETY: as above.
  unsafe { widget.set_data(FOLLOWED, true) };

  widget.connect_state_flags_changed(|widget, _| {
    if let Some(style) = slot(widget).style.borrow().clone() {
      apply_widget_props(widget, &style);
    }
  });
}

/// Gives `widget` GTK's `:active` while a primary press holds on it, once, as a button has: a
/// style varying under [`State::Pressed`] on a box that is not a button.
fn press_on_click(widget: &gtk::Widget) {
  const PRESS: &str = "skene-press";

  // SAFETY: the key only ever holds a `bool`, written here.
  if unsafe { widget.data::<bool>(PRESS) }.is_some() {
    return;
  }

  // SAFETY: as above.
  unsafe { widget.set_data(PRESS, true) };

  let gesture = gtk::GestureClick::new();

  gesture.connect_pressed(|gesture, _, _, _| {
    if let Some(widget) = gesture.widget() {
      widget.set_state_flags(gtk::StateFlags::ACTIVE, false);
    }
  });
  gesture.connect_released(|gesture, _, _, _| {
    if let Some(widget) = gesture.widget() {
      widget.unset_state_flags(gtk::StateFlags::ACTIVE);
    }
  });
  gesture.connect_stopped(|gesture| {
    if let Some(widget) = gesture.widget() {
      widget.unset_state_flags(gtk::StateFlags::ACTIVE);
    }
  });

  widget.add_controller(gesture);
}

/// Defines a scope's color tokens as named colors, display-wide, and reloads the token sheet:
/// every class naming one repaints, and no widget is touched.
pub fn define_tokens(scope: ScopeId, tokens: &[KeptDefinition]) {
  let lines: String = tokens
    .iter()
    .filter_map(|definition| match &definition.value.base {
      KeptLeaf::Value(Resolved::Color(color)) => Some(format!(
        "@define-color {} {};\n",
        token_name(definition.name, definition.tag),
        css_color(*color)
      )),
      _ => None,
    })
    .collect();
  let (provider, css) = with_sheet(|sheet| {
    if tokens.is_empty() {
      sheet.colors.remove(&scope);
    } else {
      sheet.colors.insert(scope, lines);
    }

    (
      sheet.tokens.clone(),
      sheet.colors.values().cloned().collect::<String>(),
    )
  });

  count(|stats| stats.token_reloads += 1);
  provider.load_from_string(&css);
}

/// Drops the sheet references of `widget` and everything beneath it.
pub fn release(widget: &gtk::Widget) {
  if slot(widget).target.borrow().is_some() {
    retarget(widget, None);
  }

  for child in children(widget) {
    release(&child);
  }
}
