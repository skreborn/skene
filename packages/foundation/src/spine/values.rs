mod access;
mod aspect_ratio;
mod axis;
mod check_state;
mod clipboard;
mod color;
mod dimension;
mod flex;
mod focus_order;
mod grid;
mod inset;
mod layout;
mod link;
mod mouse_cursor;
mod overflow;
mod placement;
mod point;
mod rect;
mod role;
mod run_display;
mod size;
mod text_decoration;
mod text_selection;
mod visibility;

pub use access::*;
pub use aspect_ratio::*;
pub use axis::*;
pub use check_state::*;
pub use clipboard::*;
pub use color::*;
pub use dimension::*;
pub use flex::*;
pub use focus_order::*;
pub use grid::*;
pub use inset::*;
pub use layout::*;
pub use link::*;
pub use mouse_cursor::*;
pub use overflow::*;
pub use placement::*;
pub use point::*;
pub use rect::*;
pub use role::*;
pub use run_display::*;
pub use size::*;
pub use text_decoration::*;
pub use text_selection::*;
pub use visibility::*;

/// Whether `name` is ASCII letters, digits, `_` and `-`, not starting with a digit.
pub(in crate::spine) const fn is_identifier(name: &str) -> bool {
  let bytes = name.as_bytes();

  if bytes.is_empty() || bytes[0].is_ascii_digit() {
    return false;
  }

  let mut index = 0;

  while index < bytes.len() {
    let byte = bytes[index];

    if !(byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-') {
      return false;
    }

    index += 1;
  }

  true
}

macro_rules! impl_ops {
  ($type:ident { $($field:ident),+ $(,)? }) => {
    impl<T: ::std::ops::Add<Output = T>> ::std::ops::Add for $type<T> {
      type Output = Self;

      #[inline(always)]
      fn add(self, rhs: Self) -> Self {
        Self {
          $($field: self.$field + rhs.$field,)+
        }
      }
    }

    impl<T: ::std::ops::Sub<Output = T>> ::std::ops::Sub for $type<T> {
      type Output = Self;

      #[inline(always)]
      fn sub(self, rhs: Self) -> Self {
        Self {
          $($field: self.$field - rhs.$field,)+
        }
      }
    }

    impl<T: ::std::ops::Add<Output = T> + Copy> ::std::ops::AddAssign for $type<T> {
      #[inline(always)]
      fn add_assign(&mut self, rhs: Self) {
        $(self.$field = self.$field + rhs.$field;)+
      }
    }

    impl<T: ::std::ops::Sub<Output = T> + Copy> ::std::ops::SubAssign for $type<T> {
      #[inline(always)]
      fn sub_assign(&mut self, rhs: Self) {
        $(self.$field = self.$field - rhs.$field;)+
      }
    }

    impl<T: ::std::ops::Mul<Output = T> + Copy> ::std::ops::Mul<T> for $type<T> {
      type Output = Self;

      #[inline(always)]
      fn mul(self, rhs: T) -> Self {
        Self {
          $($field: self.$field * rhs,)+
        }
      }
    }

    impl<T: ::std::ops::Div<Output = T> + Copy> ::std::ops::Div<T> for $type<T> {
      type Output = Self;

      #[inline(always)]
      fn div(self, rhs: T) -> Self {
        Self {
          $($field: self.$field / rhs,)+
        }
      }
    }

    impl<T: ::std::ops::Mul<Output = T> + Copy> ::std::ops::MulAssign<T> for $type<T> {
      #[inline(always)]
      fn mul_assign(&mut self, rhs: T) {
        $(self.$field = self.$field * rhs;)+
      }
    }

    impl<T: ::std::ops::Div<Output = T> + Copy> ::std::ops::DivAssign<T> for $type<T> {
      #[inline(always)]
      fn div_assign(&mut self, rhs: T) {
        $(self.$field = self.$field / rhs;)+
      }
    }
  };
}

use impl_ops;
