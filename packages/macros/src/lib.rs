//! Proc-macros for the skene entry and the reactive store derive.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, ItemFn, parse_macro_input};

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

    #[doc(hidden)]
    #[cfg_attr(
      all(target_arch = "wasm32", target_os = "unknown"),
      ::skene_gui::wasm_bindgen::prelude::wasm_bindgen(start, wasm_bindgen = ::skene_gui::wasm_bindgen)
    )]
    pub fn __skene_main() {
      #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
      ::skene_gui::launch(#name);

      #[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
      ::skene_gui::render_main(#name);

      #[cfg(not(target_arch = "wasm32"))]
      ::skene_gui::launch(#name);
    }
  }
  .into()
}

/// Derives store lenses: one accessor per named field, available on every store
/// location whose value is this struct.
///
/// Generates a `<Name>StoreFields` trait with one method per field, blanket-implemented
/// for every `StoreField<Value = Name>`—so the accessors compose through nesting and
/// collections (`store.settings().volume()`, `store.users().key(id).name()`). The
/// generated code names `::skene_reactive`, so the deriving crate needs it as a direct
/// dependency.
#[proc_macro_derive(Store)]
pub fn derive_store(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  let name = &input.ident;
  let visibility = &input.vis;

  if !input.generics.params.is_empty() {
    return syn::Error::new_spanned(
      &input.generics,
      "#[derive(Store)] does not support generic structs yet",
    )
    .to_compile_error()
    .into();
  }

  let Data::Struct(data) = &input.data
  else {
    return syn::Error::new_spanned(&input, "#[derive(Store)] supports only structs")
      .to_compile_error()
      .into();
  };

  let Fields::Named(fields) = &data.fields
  else {
    return syn::Error::new_spanned(&data.fields, "#[derive(Store)] requires named fields")
      .to_compile_error()
      .into();
  };

  let trait_name = quote::format_ident!("{name}StoreFields");
  let trait_doc = format!(
    "Store lenses for [`{name}`]'s fields; implemented by every store location holding one.",
  );
  let mut declarations = Vec::new();
  let mut implementations = Vec::new();

  for (ordinal, field) in fields.named.iter().enumerate() {
    let Some(ident) = field.ident.as_ref()
    else {
      continue;
    };

    let ty = &field.ty;
    let Ok(ordinal) = u32::try_from(ordinal)
    else {
      continue;
    };

    let doc = format!("Addresses the `{ident}` field.");

    declarations.push(quote! {
      #[doc = #doc]
      fn #ident(self) -> ::skene_reactive::Subfield<Self, #ty>;
    });

    implementations.push(quote! {
      fn #ident(self) -> ::skene_reactive::Subfield<Self, #ty> {
        ::skene_reactive::Subfield::new(
          self,
          #ordinal,
          |value: &#name| &value.#ident,
          |value: &mut #name| &mut value.#ident,
        )
      }
    });
  }

  quote! {
    #[doc = #trait_doc]
    #[automatically_derived]
    #visibility trait #trait_name: ::skene_reactive::StoreField<Value = #name> + Sized {
      #(#declarations)*
    }

    #[automatically_derived]
    impl<S: ::skene_reactive::StoreField<Value = #name>> #trait_name for S {
      #(#implementations)*
    }
  }
  .into()
}
