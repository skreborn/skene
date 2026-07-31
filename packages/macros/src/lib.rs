//! Proc-macros for the skene entry.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

/// Marks the application entry point.
///
/// Apply to a function returning a view. Expands to the selected platform's entry glue,
/// which calls `skene::launch` with the function, so the app runs on whatever GUI
/// platform the build targets.
///
/// ```ignore
/// #[skene::main]
/// fn app() -> impl IntoView {
///   Text::new("hello")
/// }
/// ```
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let function = parse_macro_input!(item as ItemFn);
  let name = &function.sig.ident;

  quote! {
    #function

    #[doc(hidden)]
    #[cfg_attr(
      target_arch = "wasm32",
      ::skene::wasm_bindgen::prelude::wasm_bindgen(start, wasm_bindgen = ::skene::wasm_bindgen)
    )]
    pub fn __skene_main() {
      ::skene::launch(#name);
    }
  }
  .into()
}
