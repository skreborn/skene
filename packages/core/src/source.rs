use skene_reactive::Effect;

/// The source of a property value.
#[must_use]
pub enum Source<T> {
  /// A static value.
  Static(T),
  /// A derived value.
  Derived(Box<dyn Fn() -> T>),
}

impl<T: 'static> Source<T> {
  /// Applies the value, re-applying on every change when derived.
  #[inline(always)]
  pub fn bind(self, apply: impl Fn(T) + 'static) {
    match self {
      Source::Static(value) => apply(value),
      Source::Derived(derive) => Effect::new(move |_| apply(derive())),
    }
  }
}

impl<T> Source<T> {
  /// Returns whether the source is derived.
  #[inline(always)]
  pub fn is_derived(&self) -> bool {
    matches!(self, Source::Derived(_))
  }
}

impl<T: Clone> Source<T> {
  /// Reads the current value.
  #[inline(always)]
  pub fn get(&self) -> T {
    match self {
      Source::Static(value) => value.clone(),
      Source::Derived(compute) => compute(),
    }
  }
}
