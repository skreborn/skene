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
      all(target_arch = "wasm32", target_os = "unknown"),
      ::skene::wasm_bindgen::prelude::wasm_bindgen(start, wasm_bindgen = ::skene::wasm_bindgen)
    )]
    pub fn __skene_main() {
      #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
      ::skene::launch(#name);

      #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
      ::skene::render_main(#name);

      #[cfg(not(target_arch = "wasm32"))]
      ::skene::launch(#name);
    }
  }
  .into()
}
