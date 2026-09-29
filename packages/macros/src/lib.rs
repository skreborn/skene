//! Proc-macros for the skene entry, lazy loading, and the reactive store derive.

mod snapshot;
mod store;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, FnArg, ItemFn, Pat, ReturnType, parse_macro_input};

/// Marks the application entry point.
///
/// Apply to a function returning a view. Expands to the selected platform's entry glue,
/// which calls `skene_gui::launch` with the function, so the app runs on whatever GUI
/// platform the build targets.
///
/// ```ignore
/// #[skene_gui::main]
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

    // The split tool roots its call graph at a function named `main`; a wasm library
    // has none, so the entry exports an empty one, except to its own tests, whose
    // harness brings the entry.
    #[cfg(all(target_arch = "wasm32", target_os = "unknown", not(test)))]
    #[unsafe(no_mangle)]
    extern "C" fn main() {}

    #[doc(hidden)]
    #[allow(unexpected_cfgs)]
    #[cfg_attr(
      all(target_arch = "wasm32", target_os = "unknown"),
      ::skene_gui::wasm_bindgen::prelude::wasm_bindgen(start, wasm_bindgen = ::skene_gui::wasm_bindgen)
    )]
    pub fn __skene_main() {
      #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
      ::skene_gui::launch(#name);

      #[cfg(skene_platform = "server")]
      ::skene_gui::render_main(env!("CARGO_CRATE_NAME"), #name);

      #[cfg(not(any(all(target_arch = "wasm32", target_os = "unknown"), skene_platform = "server")))]
      ::skene_gui::launch(#name);
    }
  }
  .into()
}

/// Marks an async function as lazily loaded: on the web its body splits into a
/// separately-fetched wasm module, downloaded on the first call.
///
/// Apply to a plain `async fn` with owned arguments; calling it awaits the module,
/// then the body. Functions sharing a module name—`#[lazy(admin)]`—split into modules
/// that share their common code; a bare `#[lazy]` names the module after the function.
/// Splitting happens in `skene build web` when `skene.toml` sets `[web] split = true`,
/// which a build carrying lazy functions requires; every other target calls the body
/// directly.
///
/// Pair with [`Lazy`](../skene_gui/struct.Lazy.html) to render a lazy function's view:
/// `Lazy::new(settings)`.
#[expect(
  clippy::too_many_lines,
  reason = "one expansion start to end; length is the generated surface, not complexity"
)]
#[proc_macro_attribute]
pub fn lazy(args: TokenStream, item: TokenStream) -> TokenStream {
  let function = parse_macro_input!(item as ItemFn);
  let module = if args.is_empty() {
    function.sig.ident.to_string()
  }
  else {
    parse_macro_input!(args as syn::Ident).to_string()
  };

  if function.sig.asyncness.is_none() {
    return syn::Error::new_spanned(&function.sig, "#[lazy] requires an async function")
      .to_compile_error()
      .into();
  }

  if !function.sig.generics.params.is_empty() {
    return syn::Error::new_spanned(
      &function.sig.generics,
      "#[lazy] does not support generic functions",
    )
    .to_compile_error()
    .into();
  }

  if let Some(receiver) = function
    .sig
    .inputs
    .iter()
    .find(|input| matches!(input, FnArg::Receiver(_)))
  {
    return syn::Error::new_spanned(receiver, "#[lazy] does not support methods")
      .to_compile_error()
      .into();
  }

  let name = &function.sig.ident;
  // The split symbols carry a per-expansion tag, so equally-named functions in one
  // module stay distinct; underscore-free, as the split tool parses on underscores.
  let tag = {
    use std::hash::{DefaultHasher, Hash, Hasher};

    let mut hasher = DefaultHasher::new();

    format!("{name} {:?}", name.span()).hash(&mut hasher);

    format!("{:016x}", hasher.finish())
  };
  let export = format_ident!("__wasm_split_00___{module}___00_export_{tag}_{name}");
  let import = format_ident!("__wasm_split_00___{module}___00_import_{tag}_{name}");
  let load = format_ident!("__wasm_split_load_{module}_{tag}_{name}");
  let loader = format_ident!("__wasm_split_loader_{module}");

  // The wrapper keeps the function's signature; the body moves into the split-off
  // export, reached through the import once the module is loaded.
  let mut wrapper_signature = function.sig.clone();
  let mut split_signature = function.sig.clone();

  split_signature.asyncness = None;
  split_signature.output = match &function.sig.output {
    ReturnType::Default => syn::parse_quote! {
      -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = ()>>>
    },
    ReturnType::Type(_, ty) => syn::parse_quote! {
      -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = #ty>>>
    },
  };

  let mut arguments = Vec::new();

  for (position, input) in wrapper_signature.inputs.iter_mut().enumerate() {
    if let FnArg::Typed(typed) = input {
      let argument = format_ident!("__skene_lazy_argument_{position}");

      arguments.push(argument.clone());
      *typed.pat = Pat::Ident(syn::PatIdent {
        attrs: Vec::new(),
        by_ref: None,
        mutability: None,
        ident: argument,
        subpat: None,
      });
    }
  }

  let export_signature = syn::Signature {
    ident: export.clone(),
    ..split_signature.clone()
  };
  let import_signature = syn::Signature {
    ident: import.clone(),
    ..split_signature
  };
  let attributes = &function.attrs;
  let visibility = &function.vis;
  let statements = &function.block.stmts;

  quote! {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    #(#attributes)*
    #visibility #wrapper_signature {
      #[expect(improper_ctypes_definitions)]
      #[unsafe(no_mangle)]
      pub unsafe extern "C" #export_signature {
        Box::pin(async move { #(#statements)* })
      }

      #[link(wasm_import_module = "./__wasm_split.js")]
      unsafe extern "C" {
        fn #load(
          callback: unsafe extern "C" fn(*const ::std::ffi::c_void, bool),
          payload: *const ::std::ffi::c_void,
        );

        #[expect(improper_ctypes)]
        #import_signature;
      }

      ::std::thread_local! {
        static #loader: ::skene_gui::split::SplitLoader =
          unsafe { ::skene_gui::split::SplitLoader::new(#load) };
      }

      ::skene_gui::split::SplitLoader::ensure(&#loader).await;

      unsafe { #import(#(#arguments),*) }.await
    }

    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    #function
  }
  .into()
}

/// Derives store lenses: one accessor per field, available on every store location
/// whose value is this struct.
///
/// Generates a `<Name>StoreFields` trait with one method per field, blanket-implemented
/// for every `StoreField<Value = Name>`—so the accessors compose through nesting and
/// collections (`store.settings().volume()`, `store.users().key(id).name()`). Generic
/// structs and tuple structs derive too: a tuple field `0` is addressed as `field_0`.
/// A field named after a `StoreField` or `Clone` method (`get`, `set`, `with`,
/// `update`, `track`, `core`, `location`, the `try_` and `_untracked` variants,
/// `clone`) is addressed as `<name>_field`, so the accessor never shadows the
/// location's own surface.
///
/// A field's location key is a hash of its name, stable under reordering;
/// `#[store(tag = "name")]` keys it by `name` instead, so the location survives a
/// rename, and `#[store(skip)]` generates no accessor for the field. The generated code
/// names `skene_reactive` through the deriving crate's direct dependency on it, or
/// through the `reactive` re-export of the facade (`skene-gui`, `skene-tui`) or
/// `skene-common` it depends on.
#[proc_macro_derive(Store, attributes(store))]
pub fn derive_store(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);

  store::derive(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

/// Derives [`Snapshot`](../skene_reactive/trait.Snapshot.html): a struct records its
/// fields by key and an enum its variant by key with the value it carries, so a signal
/// holding the type takes part in state snapshots.
///
/// A key is a hash of the field's or the variant's name, stable under reordering;
/// `#[snapshot(tag = "name")]` keys it by `name` instead, so it survives a rename.
/// `#[snapshot(skip)]` leaves a field out of the record and restores it to its absent
/// value, and `#[snapshot(default)]` on a unit variant names the variant an enum
/// restores to where the recorded one is gone. A missing field takes its type's absent
/// value, an unknown one is skipped, and a value of another kind coerces where it
/// plainly survives. The generated code names `skene_reactive` as the `Store` derive
/// does.
#[proc_macro_derive(Snapshot, attributes(snapshot))]
pub fn derive_snapshot(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);

  snapshot::derive(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}
