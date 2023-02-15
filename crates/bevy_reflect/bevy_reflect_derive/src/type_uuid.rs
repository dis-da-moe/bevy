extern crate proc_macro;

use bevy_macro_utils::BevyManifest;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::*;
use uuid::Uuid;

/// Parses input from a derive of `TypeUuid`.
pub(crate) fn type_uuid_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast: DeriveInput = syn::parse(input).unwrap();
    // Build the trait implementation
    let type_ident = ast.ident;

    let mut uuid = None;
    for attribute in ast.attrs.iter().filter_map(|attr| attr.parse_meta().ok()) {
        let Meta::NameValue(name_value) = attribute else {
            continue;
        };

        if name_value
            .path
            .get_ident()
            .map(|i| i != "uuid")
            .unwrap_or(true)
        {
            continue;
        }

        let uuid_str = match name_value.lit {
            Lit::Str(lit_str) => lit_str,
            _ => panic!("`uuid` attribute must take the form `#[uuid = \"xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx\"`."),
        };

        uuid = Some(
            Uuid::parse_str(&uuid_str.value())
                .expect("Value specified to `#[uuid]` attribute is not a valid UUID."),
        );
    }

    let uuid =
        uuid.expect("No `#[uuid = \"xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx\"` attribute found.");
    gen_impl_type_uuid(TypeUuidDef {
        type_ident,
        generics: ast.generics,
        uuid,
    })
}

/// Generates an implementation of `TypeUuid`. If there any generics, the `TYPE_UUID` will be a composite of the generic types' `TYPE_UUID`.
pub(crate) fn gen_impl_type_uuid(def: TypeUuidDef) -> proc_macro::TokenStream {
    let uuid = def.uuid;
    let mut generics = def.generics;
    let ty = def.type_ident;

    let bevy_reflect_path: Path = BevyManifest::default().get_path("bevy_reflect");

    generics.type_params_mut().for_each(|param| {
        param
            .bounds
            .push(syn::parse_quote!(#bevy_reflect_path::TypeUuid));
    });

    let bytes = uuid
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:#X}"))
        .map(|byte_str| syn::parse_str::<LitInt>(&byte_str).unwrap());

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let base = quote! { #bevy_reflect_path::Uuid::from_bytes([#( #bytes ),*]) };
    let type_uuid = generics.type_params().enumerate().fold(base, |acc, (index, param)| {
        let ident = &param.ident;
        let param_uuid = quote!(
            #bevy_reflect_path::Uuid::from_u128(<#ident as #bevy_reflect_path::TypeUuid>::TYPE_UUID.as_u128().wrapping_add(#index as u128))
        );
        quote! {
            #bevy_reflect_path::__macro_exports::generate_composite_uuid(#acc, #param_uuid)
        }
    });

    let gen = quote! {
        impl #impl_generics #bevy_reflect_path::TypeUuid for #ty #type_generics #where_clause {
            const TYPE_UUID: #bevy_reflect_path::Uuid = #type_uuid;
        }
    };
    gen.into()
}

/// A struct containing the data required to generate an implementation of `TypeUuid`. This can be generated by either [`impl_type_uuid!`][crate::impl_type_uuid!] or [`type_uuid_derive`].
pub(crate) struct TypeUuidDef {
    pub type_ident: Ident,
    pub generics: Generics,
    pub uuid: Uuid,
}

impl Parse for TypeUuidDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let type_ident = input.parse::<Ident>()?;
        let generics = input.parse::<Generics>()?;
        input.parse::<Token![,]>()?;
        let uuid = input.parse::<LitStr>()?.value();
        let uuid = Uuid::parse_str(&uuid).map_err(|err| input.error(format!("{}", err)))?;

        Ok(Self {
            type_ident,
            generics,
            uuid,
        })
    }
}
