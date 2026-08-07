mod axis;
mod border;
mod color;
mod dimension;
mod flex;
mod grid;
mod inset;
mod layout;
mod mouse_cursor;
mod overflow;
mod placement;
mod point;
mod rect;
mod role;
mod size;

pub use axis::*;
pub use border::*;
pub use color::*;
pub use dimension::*;
pub use flex::*;
pub use grid::*;
pub use inset::*;
pub use layout::*;
pub use mouse_cursor::*;
pub use overflow::*;
pub use placement::*;
pub use point::*;
pub use rect::*;
pub use role::*;
pub use size::*;

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
