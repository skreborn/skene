//! What the choice groups share: what they have chosen, and their options, each carrying a
//! value of the app's own type.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
  Effect, Signal,
  behavior::{Choice, Controlled, Options},
  seam::Realization,
  untrack,
  view::{Label, Wraps},
};

/// What supplies an option's name.
type Name = Rc<dyn Fn() -> Rc<str>>;

/// What makes an option's caption as it is built.
type Caption<R> = Rc<dyn Fn() -> Label<R>>;

/// What captions each option of a derived run, from its value.
type Captioner<T, R> = Rc<dyn Fn(&T) -> Label<R>>;

/// A fixed run's option: its value, what supplies its name, and its caption: a text built as
/// often as asked, a view until it is built.
type FixedOption<T, R> = (T, Option<Name>, FixedCaption<R>);

/// A fixed option's caption: a text, or a view given once, until it is built.
enum FixedCaption<R: Realization> {
  Text(Rc<str>),
  Once(Rc<RefCell<Option<Label<R>>>>),
}

/// One run of a choice group's options: given one by one or as a list, or derived from the
/// app's data. A group's options are its runs, in order.
pub enum OptionSegment<T: Choice, R: Realization> {
  /// Options given as they are, each with its caption.
  Fixed(Vec<(T, Label<R>)>),
  /// Options derived: the values `values` returns, each captioned by `label`, kept current.
  Derived {
    /// The options' values.
    values: Rc<dyn Fn() -> Vec<T>>,
    /// What captions each.
    label: Captioner<T, R>,
  },
}

/// One option as a group's runs give it, with whether its caption is a view given once.
struct Entry<T, R: Realization> {
  value: T,
  name: Option<Name>,
  caption: Caption<R>,
  once: bool,
}

/// A group's options in order, each value's place found by a map.
struct List<T, R: Realization> {
  entries: Vec<Entry<T, R>>,
  places: HashMap<T, usize>,
}

/// A choice group's options as its runs give them, kept current as a derived run changes:
/// what its roster moves through, and what builds each option's caption.
pub struct GroupOptions<T: Choice, R: Realization> {
  list: Signal<Rc<List<T, R>>>,
}

impl<T: Choice, R: Realization> Clone for GroupOptions<T, R> {
  fn clone(&self) -> Self {
    Self { list: self.list }
  }
}

impl<T: Choice, R: Realization> GroupOptions<T, R> {
  /// The options `segments` give, kept current under the running owner.
  ///
  /// # Panics
  ///
  /// In a debug build, where two options carry one value.
  pub fn new(segments: Vec<OptionSegment<T, R>>) -> Self {
    let list = Signal::new(Rc::new(List {
      entries: Vec::new(),
      places: HashMap::new(),
    }));
    let segments: Vec<Run<T, R>> = segments.into_iter().map(Run::of).collect();

    Effect::new(move || {
      let mut entries = Vec::new();

      for run in &segments {
        run.entries(&mut entries);
      }

      let mut places = HashMap::with_capacity(entries.len());

      for (index, entry) in entries.iter().enumerate() {
        let previous = places.insert(entry.value.clone(), index);

        debug_assert!(
          previous.is_none(),
          "two options of one choice carry one value"
        );
      }

      list.set(Rc::new(List { entries, places }));
    });

    Self { list }
  }

  /// The options' values in order, tracking them.
  #[must_use]
  pub fn values(&self) -> Vec<T> {
    self.list.with(|list| {
      list
        .entries
        .iter()
        .map(|entry| entry.value.clone())
        .collect()
    })
  }

  /// The caption of the option carrying `value`, as it is built: its own, a view given once
  /// shown by a text of its name where it was built already.
  pub fn caption(&self, value: &T) -> Label<R> {
    let caption = untrack(|| {
      self.list.with(|list| {
        list
          .places
          .get(value)
          .map(|index| Rc::clone(&list.entries[*index].caption))
      })
    });

    caption.map_or_else(|| Label::from(""), |caption| caption())
  }

  /// Whether the option carrying `value` is captioned by a view given once, which can be built
  /// only once: a list building its options as they come into view keeps it built from then
  /// on.
  #[must_use]
  pub fn caption_once(&self, value: &T) -> bool {
    untrack(|| {
      self.list.with(|list| {
        list
          .places
          .get(value)
          .is_some_and(|index| list.entries[*index].once)
      })
    })
  }

  /// The name of the option carrying `value`, where its caption states one, tracking it.
  #[must_use]
  pub fn name_of(&self, value: &T) -> Option<Rc<str>> {
    self.index_of(value).and_then(|index| self.name(index))
  }
}

impl<T: Choice, R: Realization> Options<T> for GroupOptions<T, R> {
  fn count(&self) -> usize {
    self.list.with(|list| list.entries.len())
  }

  fn value(&self, index: usize) -> Option<T> {
    self
      .list
      .with(|list| list.entries.get(index).map(|entry| entry.value.clone()))
  }

  fn index_of(&self, value: &T) -> Option<usize> {
    self.list.with(|list| list.places.get(value).copied())
  }

  fn name(&self, index: usize) -> Option<Rc<str>> {
    let name = self
      .list
      .with(|list| list.entries.get(index).and_then(|entry| entry.name.clone()));

    name.map(|name| name())
  }
}

/// A run as a group keeps it once built: a fixed run's text captions made for each build and
/// its view captions taken as their options are built, a derived run's made for each.
enum Run<T: Choice, R: Realization> {
  Fixed(Vec<FixedOption<T, R>>),
  Derived {
    values: Rc<dyn Fn() -> Vec<T>>,
    label: Captioner<T, R>,
  },
}

impl<T: Choice, R: Realization> Run<T, R> {
  fn of(segment: OptionSegment<T, R>) -> Self {
    match segment {
      OptionSegment::Fixed(options) => Self::Fixed(
        options
          .into_iter()
          .map(|(value, label)| {
            let name = label.name_source();
            let caption = match label.text() {
              Some(text) => FixedCaption::Text(text),
              None => FixedCaption::Once(Rc::new(RefCell::new(Some(label)))),
            };

            (value, name, caption)
          })
          .collect(),
      ),
      OptionSegment::Derived { values, label } => Self::Derived { values, label },
    }
  }

  /// Appends the run's options to `entries`, tracking a derived run's values.
  fn entries(&self, entries: &mut Vec<Entry<T, R>>) {
    match self {
      Self::Fixed(options) => {
        for (value, name, caption) in options {
          let named = name.clone();
          let (caption, once): (Caption<R>, bool) = match caption {
            FixedCaption::Text(text) => {
              let text = Rc::clone(text);

              (
                Rc::new(move || {
                  let label = Label::from(Rc::clone(&text));

                  match &named {
                    Some(name) => {
                      let name = Rc::clone(name);

                      label.derive_name(move || name())
                    }
                    None => label,
                  }
                }),
                false,
              )
            }
            FixedCaption::Once(label) => {
              let label = Rc::clone(label);

              (
                Rc::new(move || {
                  label.borrow_mut().take().unwrap_or_else(|| {
                    Label::from(named.as_ref().map_or_else(|| Rc::from(""), |name| name()))
                  })
                }),
                true,
              )
            }
          };

          entries.push(Entry {
            value: value.clone(),
            name: name.clone(),
            caption,
            once,
          });
        }
      }
      Self::Derived { values, label } => {
        for value in values() {
          let name = untrack(|| label(&value).name_source());
          let (label, captioned) = (Rc::clone(label), value.clone());

          entries.push(Entry {
            value,
            name,
            caption: Rc::new(move || label(&captioned)),
            once: false,
          });
        }
      }
    }
  }
}

/// A view over a choice: what [`ChoiceBase`] writes.
pub trait HasChoice<V: Clone + 'static> {
  /// Changes its controlled choice before it is built.
  fn change_chosen(&mut self, change: impl FnOnce(&mut Controlled<V>));
}

/// What every choice group carries: what it has chosen, `V`, controlled. A picker and a radio
/// group choose one of their options' values; a list box an `Option` of one, or a set.
pub trait ChoiceBase<V: Clone + 'static>: Wraps<Inner: HasChoice<V>> {
  /// What is chosen.
  fn set_chosen(mut self, chosen: V) -> Self {
    self
      .inner_mut()
      .change_chosen(|controlled| controlled.set(chosen));

    self
  }

  /// What is chosen, kept current.
  fn derive_chosen(mut self, chosen: impl Fn() -> V + 'static) -> Self {
    self
      .inner_mut()
      .change_chosen(|controlled| controlled.derive(chosen));

    self
  }

  /// Runs `choose` with each choice the reader makes; a choice it refuses never shows.
  fn handle_choose(mut self, choose: impl Fn(V) + 'static) -> Self {
    self
      .inner_mut()
      .change_chosen(|controlled| controlled.handle(choose));

    self
  }

  /// Shows `chosen` and writes each choice into it.
  fn bind_chosen(mut self, chosen: Signal<V>) -> Self {
    self
      .inner_mut()
      .change_chosen(|controlled| controlled.bind_signal(chosen));

    self
  }
}

/// A view over a choice group's options: what [`OptionsBase`] writes.
pub trait HasOptions<T: Choice, R: Realization> {
  /// Its options' runs, to append to or replace.
  fn segments_mut(&mut self) -> &mut Vec<OptionSegment<T, R>>;
}

/// What every choice group's options are: runs of options each carrying a `T` and captioned by
/// a label, given one by one, as a list, or derived from the app's data. They are its content,
/// so they end its chain. A debug build panics where two options carry one value.
pub trait OptionsBase<T: Choice, R: Realization>: Wraps<Inner: HasOptions<T, R>> {
  /// Appends an option carrying `value`, captioned by `label`: a text names it, a view by the
  /// texts it shows once built, or through [`Label::set_name`], which type-ahead finds before
  /// the option is built.
  fn add_option(mut self, value: T, label: impl Into<Label<R>>) -> Self {
    let segments = self.inner_mut().segments_mut();

    match segments.last_mut() {
      Some(OptionSegment::Fixed(options)) => options.push((value, label.into())),
      _ => segments.push(OptionSegment::Fixed(vec![(value, label.into())])),
    }

    self
  }

  /// Its options, in place of those it has.
  fn set_options<L: Into<Label<R>>>(mut self, options: impl IntoIterator<Item = (T, L)>) -> Self {
    *self.inner_mut().segments_mut() = vec![OptionSegment::Fixed(
      options
        .into_iter()
        .map(|(value, label)| (value, label.into()))
        .collect(),
    )];

    self
  }

  /// Appends the options `values` returns, kept current: each value's option is built once,
  /// captioned by `label`, and kept, wherever it moves, while the value stays.
  fn derive_options<L: Into<Label<R>>>(
    mut self,
    values: impl Fn() -> Vec<T> + 'static,
    label: impl Fn(&T) -> L + 'static,
  ) -> Self {
    self
      .inner_mut()
      .segments_mut()
      .push(OptionSegment::Derived {
        values: Rc::new(values),
        label: Rc::new(move |value| label(value).into()),
      });

    self
  }
}
