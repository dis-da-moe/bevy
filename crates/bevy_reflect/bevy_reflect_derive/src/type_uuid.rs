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
    let mut ast: DeriveInput = syn::parse(input).unwrap();
    let bevy_reflect_path: Path = BevyManifest::default().get_path("bevy_reflect");
    // Build the trait implementation
    let name = ast.ident;
    let path = Path::from(name);
    let type_path = TypePath { qself: None, path };
    let r#type = Type::Path(type_path);
    ast.generics.type_params_mut().for_each(|param| {
        param
            .bounds
            .push(syn::parse_quote!(#bevy_reflect_path::TypeUuid));
    });

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
        r#type,
        generics: ast.generics,
        uuid,
        include_type_generics: true,
    })
}

/// Generates an implementation of `TypeUuid`. If there any generics, the `TYPE_UUID` will be a composite of the generic types' `TYPE_UUID`.
pub(crate) fn gen_impl_type_uuid(def: TypeUuidDef) -> proc_macro::TokenStream {
    let uuid = def.uuid;
    let mut generics = def.generics;
    let r#type = def.r#type;

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

    let type_generics = if def.include_type_generics {
        Some(type_generics)
    } else {
        None
    };
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
        impl #impl_generics #bevy_reflect_path::TypeUuid for #r#type #type_generics #where_clause {
            const TYPE_UUID: #bevy_reflect_path::Uuid = #type_uuid;
        }
    };
    gen.into()
}

/// A struct containing the data required to generate an implementation of `TypeUuid`. This can be generated by either [`impl_type_uuid!`][crate::impl_type_uuid!] or [`type_uuid_derive`].
pub(crate) struct TypeUuidDef {
    pub r#type: Type,
    pub generics: Generics,
    pub uuid: Uuid,
    pub include_type_generics: bool,
}

impl Parse for TypeUuidDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let generics = input.parse::<Generics>()?;
        let r#type = input.parse::<Type>()?;
        input.parse::<Token![,]>()?;
        let uuid = input.parse::<LitStr>()?.value();
        let uuid = Uuid::parse_str(&uuid).map_err(|err| input.error(format!("{}", err)))?;
        let include_type_generics = match r#type {
            Type::Path(_) => true,
            _ => false,
        };

        Ok(Self {
            r#type,
            generics,
            uuid,
            include_type_generics,
        })
    }
}
