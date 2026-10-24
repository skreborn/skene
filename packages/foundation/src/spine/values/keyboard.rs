/// What a text entry takes, as the keyboard a platform offers for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub enum KeyboardType {
  /// Any text.
  #[default]
  Text,
  /// An email address.
  Email,
  /// A whole number.
  Number,
  /// A number with a fraction.
  Decimal,
  /// A telephone number.
  Phone,
  /// A URL.
  Url,
  /// A search's query.
  Search,
}

/// What a text entry holds, for the platform to fill it from what it keeps of the
/// reader: their name, their account, their password.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Autofill {
  /// The reader's full name.
  Name,
  /// The name of an account.
  Username,
  /// An email address.
  Email,
  /// A telephone number.
  Phone,
  /// A URL.
  Url,
  /// An account's password.
  Password,
  /// A password being chosen for an account.
  NewPassword,
  /// A one-time code sent to the reader.
  OneTimeCode,
}

/// Which letters the platform's keyboard capitalizes as the reader types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub enum Capitalization {
  /// As the platform does for the entry's type.
  #[default]
  Automatic,
  /// None.
  Never,
  /// Every letter.
  Characters,
  /// Each word's first letter.
  Words,
  /// Each sentence's first letter.
  Sentences,
}

/// What the keyboard's `Enter` key is labelled as doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum SubmitLabel {
  /// Done: what the entry takes is complete.
  Done,
  /// Go: to where the entry names.
  Go,
  /// Next: on to the next entry.
  Next,
  /// Previous: back to the previous entry.
  Previous,
  /// Search: for what the entry holds.
  Search,
  /// Send: what the entry holds.
  Send,
}

/// The keyboard a text entry asks the platform for: what it takes, what it may be filled
/// from, and how the input method treats what is typed, as one value every platform's
/// input protocol carries.
///
/// A part a platform's protocol has no word for is left to the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Keyboard {
  /// What the entry takes.
  pub kind: KeyboardType,
  /// What the entry may be filled from; `None` leaves it to the platform.
  pub autofill: Option<Autofill>,
  /// Whether the input method may correct and complete what is typed, as the platform
  /// does by default.
  pub autocorrect: bool,
  /// Which letters the keyboard capitalizes.
  pub capitalization: Capitalization,
  /// What the `Enter` key is labelled as doing; `None` leaves the platform's own.
  pub submit: Option<SubmitLabel>,
}

impl Keyboard {
  /// A keyboard for any text, every other part the platform's.
  pub const TEXT: Self = Self::new(KeyboardType::Text);
  /// A keyboard for an email address.
  pub const EMAIL: Self = Self::new(KeyboardType::Email);
  /// A keyboard for a whole number.
  pub const NUMBER: Self = Self::new(KeyboardType::Number);
  /// A keyboard for a number with a fraction.
  pub const DECIMAL: Self = Self::new(KeyboardType::Decimal);
  /// A keyboard for a telephone number.
  pub const PHONE: Self = Self::new(KeyboardType::Phone);
  /// A keyboard for a URL.
  pub const URL: Self = Self::new(KeyboardType::Url);
  /// A keyboard for a search's query.
  pub const SEARCH: Self = Self::new(KeyboardType::Search);

  /// A keyboard for what `kind` names, every other part the platform's.
  #[inline(always)]
  pub const fn new(kind: KeyboardType) -> Self {
    Self {
      kind,
      autofill: None,
      autocorrect: true,
      capitalization: Capitalization::Automatic,
      submit: None,
    }
  }

  /// This keyboard, the entry filled from what `autofill` names.
  #[inline(always)]
  pub const fn with_autofill(mut self, autofill: Autofill) -> Self {
    self.autofill = Some(autofill);

    self
  }

  /// This keyboard, the input method correcting and completing what is typed where
  /// `autocorrect`, and never otherwise.
  #[inline(always)]
  pub const fn with_autocorrect(mut self, autocorrect: bool) -> Self {
    self.autocorrect = autocorrect;

    self
  }

  /// This keyboard, capitalizing as `capitalization` says.
  #[inline(always)]
  pub const fn with_capitalization(mut self, capitalization: Capitalization) -> Self {
    self.capitalization = capitalization;

    self
  }

  /// This keyboard, its `Enter` key labelled as `submit` says.
  #[inline(always)]
  pub const fn with_submit(mut self, submit: SubmitLabel) -> Self {
    self.submit = Some(submit);

    self
  }
}

impl Default for Keyboard {
  #[inline(always)]
  fn default() -> Self {
    Self::TEXT
  }
}
