//! Procedural macros for skene.
//!
//! [`Builder`] derives a fluent, self-returning setter API directly on a widget
//! struct — the struct *is* its own builder, so there is no separate builder
//! type and no `.build()` finalizer to call. This is the right trade-off for
//! skene's widgets: every required input is a constructor argument, so there is
//! nothing for a typestate builder (à la `bon`) to validate.
//!
//! See [`Builder`] for the generated API and the supported field attributes.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use syn::{
  Data,
  DeriveInput,
  Field,
  Fields,
  GenericArgument,
  Ident,
  PathArguments,
  Type,
  TypeParamBound,
  parse_macro_input,
};

/// Derive fluent, self-returning setters on a struct.
///
/// The struct doubles as its own builder: each generated setter takes `self` by
/// value, sets one field, and returns `Self`. There is no separate builder type
/// and no finalizer — the value is ready to use as soon as you stop chaining.
///
/// # Which fields get setters
///
/// * `Option<T>` → a setter `fn field(self, T) -> Self` that stores `Some`.
/// * `Vec<T>` → collection setters (see below).
/// * any other type → **left alone** (treated as a required field set by a hand-written
///   constructor); no setter is generated.
///
/// By default a setter takes its value *by `T`*. Opt into an `impl Into<T>`
/// signature per field with `#[builder(into)]`.
///
/// # Field attributes (`#[builder(...)]`)
///
/// * `skip` — generate no setter for this field.
/// * `into` — take `impl Into<T>` instead of `T` (e.g. so an `EdgeInsets` field accepts `20.0`).
/// * `callback` — for an `Option<Rc<dyn Fn..>>` (or `Box`/`Arc`) field: generate a setter
///   that takes the closure directly (`impl Fn.. + 'static`) and wraps it in the pointer.
/// * `view` — convert with skene's `IntoView` (`.into_view()`); for `Vec<AnyView>` children.
/// * `item = name` — for a `Vec` field, also generate a singular adder `fn name(self, ..)`
///   that pushes one element. Without it, only the plural extender (named after the field)
///   is generated.
///
/// # Example
///
/// ```ignore
/// #[derive(Builder)]
/// pub struct Button {
///   label: String,                          // required: no setter
///   #[builder(callback)]
///   action: Option<Rc<dyn Fn()>>,            // .action(impl Fn() + 'static)
///   #[builder(into)]
///   background: Option<Color>,               // .background(impl Into<Color>)
///   weight: Option<u16>,                     // .weight(u16)
/// }
/// ```
#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive_builder(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  expand(&input)
    .unwrap_or_else(syn::Error::into_compile_error)
    .into()
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
  let name = &input.ident;
  let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

  let Data::Struct(data) = &input.data
  else {
    return Err(syn::Error::new_spanned(
      name,
      "`Builder` can only be derived for structs",
    ));
  };
  let Fields::Named(fields) = &data.fields
  else {
    return Err(syn::Error::new_spanned(
      name,
      "`Builder` requires a struct with named fields",
    ));
  };

  let core = skene_core_path();
  let mut methods = Vec::new();

  for field in &fields.named {
    let cfg = FieldConfig::parse(field)?;
    if cfg.skip {
      continue;
    }
    let field_name = field.ident.as_ref().expect("named field");
    // The field's own doc comments are carried onto the generated setter(s).
    let docs = doc_attrs(field);

    if let Some(item_ty) = generic_inner(&field.ty, "Vec") {
      methods.push(collection_setters(&cfg, &docs, field_name, item_ty, &core));
    }
    else if let Some(inner_ty) = generic_inner(&field.ty, "Option") {
      methods.push(option_setter(&cfg, &docs, field, field_name, inner_ty, &core)?);
    }
    // Any other field type is a required field owned by a constructor: skip it.
  }

  Ok(quote! {
    impl #impl_generics #name #ty_generics #where_clause {
      #(#methods)*
    }
  })
}

/// A scalar `Option<T>` setter.
fn option_setter(
  cfg: &FieldConfig,
  docs: &TokenStream2,
  field: &Field,
  name: &Ident,
  inner: &Type,
  core: &TokenStream2,
) -> syn::Result<TokenStream2> {
  if cfg.callback {
    return callback_setter(docs, field, name, inner);
  }
  if cfg.view {
    return Ok(quote! {
      #docs
      #[inline(always)]
      pub fn #name(mut self, value: impl #core::IntoView) -> Self {
        self.#name = ::core::option::Option::Some(#core::IntoView::into_view(value));
        self
      }
    });
  }
  if cfg.into {
    return Ok(quote! {
      #docs
      #[inline(always)]
      pub fn #name(mut self, value: impl ::core::convert::Into<#inner>) -> Self {
        self.#name = ::core::option::Option::Some(::core::convert::Into::into(value));
        self
      }
    });
  }
  Ok(quote! {
    #docs
    #[inline(always)]
    pub fn #name(mut self, value: #inner) -> Self {
      self.#name = ::core::option::Option::Some(value);
      self
    }
  })
}

/// A callback setter for `Option<Rc<dyn Fn..>>` (or `Box`/`Arc`): takes the
/// closure directly and wraps it in the pointer type the field stores.
fn callback_setter(
  docs: &TokenStream2,
  field: &Field,
  name: &Ident,
  inner: &Type,
) -> syn::Result<TokenStream2> {
  let err = || {
    syn::Error::new_spanned(
      &field.ty,
      "`#[builder(callback)]` requires a field of type `Option<Rc|Arc|Box<dyn Fn..>>`",
    )
  };

  let Type::Path(pointer) = inner
  else {
    return Err(err());
  };
  let segment = pointer.path.segments.last().ok_or_else(err)?;
  let wrapper = match segment.ident.to_string().as_str() {
    "Rc" => quote!(::std::rc::Rc),
    "Arc" => quote!(::std::sync::Arc),
    "Box" => quote!(::std::boxed::Box),
    _ => return Err(err()),
  };

  let PathArguments::AngleBracketed(args) = &segment.arguments
  else {
    return Err(err());
  };
  let Some(GenericArgument::Type(Type::TraitObject(object))) = args.args.first()
  else {
    return Err(err());
  };

  // Reuse the `dyn Fn..` bounds for the `impl ..` parameter, ensuring `'static`.
  let bounds = &object.bounds;
  let has_lifetime = bounds
    .iter()
    .any(|b| matches!(b, TypeParamBound::Lifetime(_)));
  let lifetime = if has_lifetime {
    quote!()
  }
  else {
    quote!(+ 'static)
  };

  Ok(quote! {
    #docs
    #[inline(always)]
    pub fn #name(mut self, #name: impl #bounds #lifetime) -> Self {
      self.#name = ::core::option::Option::Some(#wrapper::new(#name));
      self
    }
  })
}

/// A `Vec<T>` collection: a plural extender named after the field, plus an
/// optional singular adder named by `#[builder(item = ..)]`.
fn collection_setters(
  cfg: &FieldConfig,
  docs: &TokenStream2,
  field: &Ident,
  item: &Type,
  core: &TokenStream2,
) -> TokenStream2 {
  let singular = cfg.item.as_ref().map(|adder| {
    if cfg.view {
      quote! {
        #docs
        #[inline(always)]
        pub fn #adder(mut self, item: impl #core::IntoView) -> Self {
          self.#field.push(#core::IntoView::into_view(item));
          self
        }
      }
    }
    else if cfg.into {
      quote! {
        #docs
        #[inline(always)]
        pub fn #adder(mut self, item: impl ::core::convert::Into<#item>) -> Self {
          self.#field.push(::core::convert::Into::into(item));
          self
        }
      }
    }
    else {
      quote! {
        #docs
        #[inline(always)]
        pub fn #adder(mut self, item: #item) -> Self {
          self.#field.push(item);
          self
        }
      }
    }
  });

  let plural = if cfg.view {
    quote! {
      #docs
      #[inline(always)]
      pub fn #field<__SkeneItems>(mut self, items: __SkeneItems) -> Self
      where
        __SkeneItems: ::core::iter::IntoIterator,
        __SkeneItems::Item: #core::IntoView,
      {
        self.#field.extend(items.into_iter().map(#core::IntoView::into_view));
        self
      }
    }
  }
  else if cfg.into {
    quote! {
      #docs
      #[inline(always)]
      pub fn #field<__SkeneItems>(mut self, items: __SkeneItems) -> Self
      where
        __SkeneItems: ::core::iter::IntoIterator,
        __SkeneItems::Item: ::core::convert::Into<#item>,
      {
        self.#field.extend(items.into_iter().map(::core::convert::Into::into));
        self
      }
    }
  }
  else {
    quote! {
      #docs
      #[inline(always)]
      pub fn #field(mut self, items: impl ::core::iter::IntoIterator<Item = #item>) -> Self {
        self.#field.extend(items);
        self
      }
    }
  };

  quote! {
    #singular
    #plural
  }
}

#[derive(Default)]
#[allow(clippy::struct_excessive_bools)] // independent, orthogonal field flags
struct FieldConfig {
  skip: bool,
  into: bool,
  view: bool,
  callback: bool,
  item: Option<Ident>,
}

impl FieldConfig {
  fn parse(field: &Field) -> syn::Result<Self> {
    let mut cfg = FieldConfig::default();
    for attr in &field.attrs {
      if !attr.path().is_ident("builder") {
        continue;
      }
      attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("skip") {
          cfg.skip = true;
        }
        else if meta.path.is_ident("into") {
          cfg.into = true;
        }
        else if meta.path.is_ident("view") {
          cfg.view = true;
        }
        else if meta.path.is_ident("callback") {
          cfg.callback = true;
        }
        else if meta.path.is_ident("item") {
          cfg.item = Some(meta.value()?.parse()?);
        }
        else {
          return Err(meta.error("unknown `builder` attribute"));
        }
        Ok(())
      })?;
    }
    Ok(cfg)
  }
}

/// The field's doc-comment attributes (`#[doc = ".."]`), to be re-emitted on the
/// generated setter so the documentation carries through.
fn doc_attrs(field: &Field) -> TokenStream2 {
  let docs = field
    .attrs
    .iter()
    .filter(|attr| attr.path().is_ident("doc"));
  quote!(#(#docs)*)
}

/// If `ty` is `Wrapper<Inner>` (matched by the last path segment), return `Inner`.
fn generic_inner<'a>(ty: &'a Type, wrapper: &str) -> Option<&'a Type> {
  let Type::Path(path) = ty
  else {
    return None;
  };
  let segment = path.path.segments.last()?;
  if segment.ident != wrapper {
    return None;
  }
  let PathArguments::AngleBracketed(args) = &segment.arguments
  else {
    return None;
  };
  args.args.iter().find_map(|arg| match arg {
    GenericArgument::Type(t) => Some(t),
    _ => None,
  })
}

/// The path to refer to `skene-core` from generated code: `crate` when expanding
/// inside `skene-core` itself, otherwise the extern crate name.
fn skene_core_path() -> TokenStream2 {
  match crate_name("skene-core") {
    Ok(FoundCrate::Itself) => quote!(crate),
    Ok(FoundCrate::Name(name)) => {
      let ident = Ident::new(&name, Span::call_site());
      quote!(::#ident)
    }
    Err(_) => quote!(::skene_core),
  }
}
