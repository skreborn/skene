//! Proc-macros for the skene entry, lazy loading, and the store and snapshot derives.

mod snapshot;
mod store;
mod wraps;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, FnArg, ItemFn, Pat, ReturnType, parse_macro_input};

/// Marks a web app's entry point: a function returning the app's view.
///
/// Expands to the app crate's `main` for the leaf the build names: in the browser
/// (`skene_platform = "web"`) it launches the app, hydrating a server-rendered page in
/// place; on the server leaf (`skene_server` beside it) it renders the requested path to
/// stdout. The crate is the app's library alone: the `skene` command builds it as each
/// leaf's crate type, and its `check-cfg` declares the two cfgs the expansion reads.
///
/// The attribute takes no arguments and the function no parameters and no generics:
/// the build names the platform.
///
/// ```ignore
/// #[skene_web::main]
/// fn app() -> impl View {
///   Text::new("hello")
/// }
/// ```
#[proc_macro_attribute]
pub fn web_main(attr: TokenStream, item: TokenStream) -> TokenStream {
  let function = match entry(attr, item) {
    Ok(function) => function,
    Err(error) => return error,
  };
  let name = &function.sig.ident;

  quote! {
    #function

    #[cfg(all(skene_platform = "web", not(skene_server)))]
    #[doc(hidden)]
    #[::skene_web::wasm_bindgen::prelude::wasm_bindgen(start, wasm_bindgen = ::skene_web::wasm_bindgen)]
    pub fn __skene_start() {
      ::skene_web::launch(#name);
    }

    // The split tool roots its call graph at a function named `main`; the entry
    // exports an empty one, except to its own tests, whose harness brings the entry.
    #[cfg(all(skene_platform = "web", not(skene_server), not(test)))]
    #[unsafe(no_mangle)]
    extern "C" fn main() {}

    #[cfg(all(skene_platform = "web", skene_server))]
    #[doc(hidden)]
    pub fn main() {
      ::skene_web::render_main(env!("CARGO_CRATE_NAME"), #name);
    }
  }
  .into()
}

/// Marks a terminal app's entry point: a function returning the app's view.
///
/// Expands to the app crate's `main` in the terminal's build (`skene_platform =
/// "terminal"`), launching the app in the terminal. The crate is the app's library
/// alone: the `skene` command builds it as a binary, and its `check-cfg` declares the
/// cfg the expansion reads.
///
/// The attribute takes no arguments and the function no parameters and no generics:
/// the build names the platform.
///
/// ```ignore
/// #[skene_terminal::main]
/// fn app() -> impl View {
///   Text::new("hello")
/// }
/// ```
#[proc_macro_attribute]
pub fn terminal_main(attr: TokenStream, item: TokenStream) -> TokenStream {
  let function = match entry(attr, item) {
    Ok(function) => function,
    Err(error) => return error,
  };
  let name = &function.sig.ident;

  quote! {
    #function

    #[cfg(skene_platform = "terminal")]
    #[doc(hidden)]
    pub fn main() {
      ::skene_terminal::launch(#name);
    }
  }
  .into()
}

/// The function an entry attribute marks: one taking no parameters and no generics,
/// under an attribute taking no arguments, since the build names the platform; else the
/// error saying why, beside the function as written.
fn entry(attr: TokenStream, item: TokenStream) -> Result<ItemFn, TokenStream> {
  let function = syn::parse::<ItemFn>(item).map_err(syn::Error::into_compile_error)?;
  let attr = proc_macro2::TokenStream::from(attr);
  let refused = if !attr.is_empty() {
    Some(syn::Error::new_spanned(
      attr,
      "an entry takes no arguments: the build names its platform",
    ))
  }
  else if !function.sig.generics.params.is_empty() || function.sig.generics.where_clause.is_some()
  {
    Some(syn::Error::new_spanned(
      &function.sig.generics,
      "an entry is not generic: it returns the app's view",
    ))
  }
  else if !function.sig.inputs.is_empty() {
    Some(syn::Error::new_spanned(
      &function.sig.inputs,
      "an entry takes no parameters: it returns the app's view",
    ))
  }
  else {
    None
  };

  match refused {
    Some(error) => {
      let error = error.into_compile_error();

      Err(quote! { #function #error }.into())
    }
    None => Ok(function),
  }
}

/// Marks an async function as lazily loaded: on the web its body splits into a
/// separately-fetched wasm module, downloaded on the first call.
///
/// Apply to a plain `async fn` with owned arguments; calling it awaits the module,
/// then the body. Functions sharing a module name—`#[lazy(admin)]`—split into modules
/// that share their common code; a bare `#[lazy]` names the module after the function.
/// Splitting happens in `skene build web` when `skene.toml` sets `[web] split = true`,
/// which a browser build carrying lazy functions requires; every other build, the
/// server leaf's among them, calls the body directly.
///
/// Pair with [`AsyncView`](../skene_web/struct.AsyncView.html) to render a lazy function's view:
/// `AsyncView::new().set_child_loader(settings)`.
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
    #[cfg(all(skene_platform = "web", not(skene_server)))]
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
        static #loader: ::skene_web::split::SplitLoader =
          unsafe { ::skene_web::split::SplitLoader::new(#load) };
      }

      ::skene_web::split::SplitLoader::ensure(&#loader).await;

      unsafe { #import(#(#arguments),*) }.await
    }

    #[cfg(not(all(skene_platform = "web", not(skene_server))))]
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
/// through the `reactive` re-export of the facade (`skene-web`, `skene-terminal`) or
/// `skene-foundation` it depends on.
#[proc_macro_derive(Store, attributes(store))]
pub fn derive_store(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);

  store::derive(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

/// Derives [`Snapshot`](../skene_reactive/trait.Snapshot.html), so a saved or tagged
/// signal can hold the type and carry it across state snapshots.
///
/// A struct records its fields by key and an enum its variant by key with the value it
/// carries.
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

/// Derives [`Wraps`](../skene_foundation/trait.Wraps.html) for a tuple struct of one
/// field, the struct holding that field as its whole content: a facade's view over the
/// foundation's.
///
/// `PhantomData` markers may follow the field, for a view over the app's types holding
/// a core that is not.
///
/// The generated code names `Wraps` through the deriving crate's direct dependency on
/// `skene-foundation`, or through the re-export of the facade (`skene-web`,
/// `skene-terminal`) it depends on.
#[proc_macro_derive(Wraps)]
pub fn derive_wraps(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);

  wraps::derive_wraps(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

/// Derives [`View`](../skene_foundation/trait.View.html) and
/// [`AnyKind`](../skene_foundation/trait.AnyKind.html) for a tuple struct of one field,
/// beside any `PhantomData` markers.
///
/// The struct builds as the view it holds, for the `Active` of the one facade
/// (`skene-web`, `skene-terminal`) the deriving crate is or depends on, so its build is
/// compiled once, in the deriving crate, and a crate reaching no facade is refused. A
/// struct whose field names none of its own generic parameters builds through a
/// function that is not generic either; one whose field does builds generic in them
/// alone. A crate reaching both facades, a crate of documents' examples, gets a build
/// for each under the cfg naming its platform.
///
/// The generated code names the foundation through the deriving crate's direct
/// dependency on `skene-foundation`.
#[proc_macro_derive(View)]
pub fn derive_view(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);

  wraps::derive_view(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}
