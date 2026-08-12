//! Lazily-realized views: asynchronously-loaded content and virtualized lists.

mod column;
#[cfg(target_arch = "wasm32")]
mod content;

pub use self::column::LazyColumn;
#[cfg(target_arch = "wasm32")]
pub use self::content::Lazy;
