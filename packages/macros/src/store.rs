//! The `Store` derive: one lens accessor per field, on a trait every store location
//! holding the type implements.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, LitStr, ext::IdentExt};

/// The path to `skene_reactive` from the deriving crate: the crate itself where it is
/// a direct dependency, else the facade's `reactive` re-export.
fn reactive() -> TokenStream {
  if let Ok(found) = crate_name("skene-reactive") {
    return match found {
      FoundCrate::Itself => quote!(::skene_reactive),
      FoundCrate::Name(name) => {
        let name = format_ident!("{name}");

        quote!(::#name)
      }
    };
  }

  for facade in ["skene-gui", "skene-tui", "skene-common"] {
    if let Ok(FoundCrate::Name(name)) = crate_name(facade) {
      let name = format_ident!("{name}");

      return quote!(::#name::reactive);
    }
  }

  quote!(::skene_reactive)
}

/// The `StoreField` and `Clone` method names an accessor must not take.
const RESERVED: &[&str] = &[
  "clone",
  "clone_from",
  "core",
  "derive",
  "get",
  "location",
  "read_untracked",
  "set",
  "track",
  "track_shallow",
  "try_derive",
  "try_get",
  "try_update",
  "try_with",
  "update",
  "with",
  "write_untracked",
];

/// One field the derive addresses.
struct Field {
  accessor: Ident,
  member: syn::Member,
  ty: syn::Type,
  key: u32,
  label: String,
}

/// FNV-1a over `name`: the field key, stable under reordering.
fn field_key(name: &str) -> u32 {
  name.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
    (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
  })
}

/// Reads a field's `#[store(...)]` attributes: whether it is skipped, and its tag.
fn field_attributes(field: &syn::Field) -> syn::Result<(bool, Option<String>)> {
  let mut skip = false;
  let mut tag = None;

  for attribute in &field.attrs {
    if !attribute.path().is_ident("store") {
      continue;
    }

    attribute.parse_nested_meta(|meta| {
      if meta.path.is_ident("skip") {
        skip = true;

        return Ok(());
      }

      if meta.path.is_ident("tag") {
        let value: LitStr = meta.value()?.parse()?;

        tag = Some(value.value());

        return Ok(());
      }

      Err(meta.error("expected `skip` or `tag = \"...\"`"))
    })?;
  }

  Ok((skip, tag))
}

/// The fields the derive addresses, with their accessor names and keys.
fn fields(data: &Fields) -> syn::Result<Vec<Field>> {
  let mut fields = Vec::new();

  for (position, field) in data.iter().enumerate() {
    let (skip, tag) = field_attributes(field)?;

    if skip {
      continue;
    }

    let (accessor, member, label) = if let Some(ident) = &field.ident {
      let name = ident.unraw().to_string();
      let accessor = if RESERVED.contains(&name.as_str()) {
        format_ident!("{name}_field")
      }
      else {
        ident.clone()
      };

      (accessor, syn::Member::Named(ident.clone()), name)
    }
    else {
      (
        format_ident!("field_{position}"),
        syn::Member::Unnamed(syn::Index::from(position)),
        position.to_string(),
      )
    };
    let key = field_key(tag.as_deref().unwrap_or(&label));

    if let Some(existing) = fields.iter().find(|existing: &&Field| existing.key == key) {
      return Err(syn::Error::new_spanned(
        field,
        format!(
          "the store key of `{label}` collides with `{}`; tag one of them",
          existing.label
        ),
      ));
    }

    fields.push(Field {
      accessor,
      member,
      ty: field.ty.clone(),
      key,
      label,
    });
  }

  Ok(fields)
}

/// Expands `#[derive(Store)]`.
pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
  let name = &input.ident;
  let visibility = &input.vis;
  let Data::Struct(data) = &input.data
  else {
    return Err(syn::Error::new_spanned(
      input,
      "#[derive(Store)] supports only structs",
    ));
  };
  let fields = fields(&data.fields)?;
  let reactive = reactive();
  let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
  let trait_name = format_ident!("{name}StoreFields");
  let trait_doc = format!(
    "Store lenses for [`{name}`]'s fields; implemented by every store location holding one.",
  );
  let mut declarations = Vec::new();
  let mut implementations = Vec::new();

  for field in &fields {
    let Field {
      accessor,
      member,
      ty,
      key,
      label,
    } = field;
    let doc = format!("Addresses the `{label}` field.");

    declarations.push(quote! {
      #[doc = #doc]
      fn #accessor(self) -> #reactive::Subfield<Self, #ty>;
    });

    implementations.push(quote! {
      fn #accessor(self) -> #reactive::Subfield<Self, #ty> {
        #reactive::Subfield::new(
          self,
          #key,
          |value: &#name #ty_generics| &value.#member,
          |value: &mut #name #ty_generics| &mut value.#member,
        )
      }
    });
  }

  let mut impl_params = input.generics.params.clone();

  impl_params.push(syn::parse_quote! {
    __Location: #reactive::StoreField<Value = #name #ty_generics>
  });

  Ok(quote! {
    #[doc = #trait_doc]
    #[automatically_derived]
    #visibility trait #trait_name #impl_generics:
      #reactive::StoreField<Value = #name #ty_generics> + Sized
    #where_clause
    {
      #(#declarations)*
    }

    #[automatically_derived]
    impl<#impl_params> #trait_name #ty_generics for __Location #where_clause {
      #(#implementations)*
    }
  })
}
