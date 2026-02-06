use proc_macro::TokenStream as Ts;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

mod field;
use field::Field;

/// This macro generates an implementation of the `FromEnv` trait for a struct.
/// See the documenetation in init4_bin_base for more details.
#[proc_macro_derive(FromEnv, attributes(from_env))]
pub fn derive(input: Ts) -> Ts {
    let input = parse_macro_input!(input as DeriveInput);

    if !matches!(input.data, syn::Data::Struct(_)) {
        syn::Error::new(
            input.ident.span(),
            "FromEnv can only be derived for structs",
        )
        .to_compile_error();
    };

    let syn::Data::Struct(data) = &input.data else {
        unreachable!()
    };

    let crate_name = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("from_env"))
        .and_then(|attr| attr.parse_args::<syn::Path>().ok())
        .unwrap_or_else(|| syn::parse_str::<syn::Path>("::init4_bin_base").unwrap());

    let tuple_like = matches!(data.fields, syn::Fields::Unnamed(_));

    if matches!(data.fields, syn::Fields::Unit) {
        syn::Error::new(
            input.ident.span(),
            "FromEnv can only be derived for structs with fields",
        )
        .to_compile_error();
    }

    let fields = match &data.fields {
        syn::Fields::Named(fields) => fields.named.iter().map(Field::try_from),
        syn::Fields::Unnamed(fields) => fields.unnamed.iter().map(Field::try_from),
        syn::Fields::Unit => unreachable!(),
    };

    let fields = match fields.collect::<Result<Vec<_>, _>>() {
        Ok(fields) => fields,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let input = Input {
        ident: input.ident.clone(),
        fields,
        crate_name,
        tuple_like,
    };

    input.expand_mod().into()
}

struct Input {
    ident: syn::Ident,

    fields: Vec<Field>,

    crate_name: syn::Path,

    tuple_like: bool,
}

impl Input {
    fn field_names(&self) -> Vec<syn::Ident> {
        self.fields
            .iter()
            .enumerate()
            .map(|(idx, field)| field.field_name(idx))
            .collect()
    }

    fn instantiate_struct(&self) -> TokenStream {
        let struct_name = &self.ident;
        let field_names = self.field_names();

        if self.tuple_like {
            return quote! {
                #struct_name(
                    #(#field_names),*
                )
            };
        }

        quote! {
            #struct_name {
                #(#field_names),*
            }
        }
    }

    fn item_from_envs(&self) -> Vec<TokenStream> {
        self.fields
            .iter()
            .enumerate()
            .map(|(idx, field)| field.expand_item_from_env(idx))
            .collect()
    }

    fn env_item_info(&self) -> Vec<TokenStream> {
        self.fields
            .iter()
            .map(|field| field.expand_env_item_info())
            .collect()
    }

    fn expand_impl(&self) -> TokenStream {
        let env_item_info = self.env_item_info();
        let struct_name = &self.ident;

        let item_from_envs = self.item_from_envs();
        let struct_instantiation = self.instantiate_struct();

        quote! {
            #[automatically_derived]
            impl FromEnv for #struct_name {
                fn inventory() -> ::std::vec::Vec<&'static EnvItemInfo> {
                    let mut items = ::std::vec::Vec::new();
                    #(
                        #env_item_info
                    )*
                    items
                }

                fn from_env() -> ::std::result::Result<Self, FromEnvErr> {
                    #(
                        #item_from_envs
                    )*

                    ::std::result::Result::Ok(#struct_instantiation)
                }
            }
        }
    }

    fn expand_mod(&self) -> TokenStream {
        let expanded_impl = self.expand_impl();
        let crate_name = &self.crate_name;

        let mod_ident =
            syn::parse_str::<syn::Ident>(&format!("__from_env_impls_{}", self.ident)).unwrap();

        quote! {
            mod #mod_ident {
                use super::*;
                use #crate_name::utils::from_env::{FromEnv, FromEnvErr, FromEnvVar, EnvItemInfo};

                #expanded_impl
            }
        }
    }
}
