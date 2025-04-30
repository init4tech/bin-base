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

    fn error_ident(&self) -> syn::Ident {
        let error_name = format!("{}EnvError", self.ident);
        syn::parse_str::<syn::Ident>(&error_name)
            .map_err(|_| {
                syn::Error::new(self.ident.span(), "Failed to parse error ident").to_compile_error()
            })
            .unwrap()
    }

    fn error_variants(&self) -> Vec<TokenStream> {
        self.fields
            .iter()
            .enumerate()
            .flat_map(|(idx, field)| field.expand_enum_variant(idx))
            .collect()
    }

    fn error_variant_displays(&self) -> Vec<TokenStream> {
        self.fields
            .iter()
            .enumerate()
            .flat_map(|(idx, field)| field.expand_variant_display(idx))
            .collect::<Vec<_>>()
    }

    fn expand_variant_sources(&self) -> Vec<TokenStream> {
        self.fields
            .iter()
            .enumerate()
            .flat_map(|(idx, field)| field.expand_variant_source(idx))
            .collect::<Vec<_>>()
    }

    fn item_from_envs(&self) -> Vec<TokenStream> {
        let error_ident = self.error_ident();
        self.fields
            .iter()
            .enumerate()
            .map(|(idx, field)| field.expand_item_from_env(&error_ident, idx))
            .collect()
    }

    fn expand_error(&self) -> TokenStream {
        let error_ident = self.error_ident();
        let struct_name_str = &self.ident.to_string();

        let error_variants = self.error_variants();
        let error_variant_displays = self.error_variant_displays();
        let error_variant_sources = self.expand_variant_sources();

        quote! {
            #[doc = "Generated error type for [`FromEnv`] for"]
            #[doc = #struct_name_str]
            #[doc = ". This error type is used to represent errors that occur when trying to create an instance of the struct from environment variables."]
            #[derive(Debug, PartialEq, Eq, Clone)]
            pub enum #error_ident {
                #(#error_variants),*
            }

            #[automatically_derived]
            impl ::core::fmt::Display for #error_ident {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    match self {
                        #(
                            #error_variant_displays,
                        )*
                    }
                }
            }

            #[automatically_derived]
            impl ::core::error::Error for #error_ident {
                fn source(&self) -> Option<&(dyn ::core::error::Error + 'static)> {
                    match self {
                        #(
                            #error_variant_sources,
                        )*
                    }
                }
            }
        }
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
        let error_ident = self.error_ident();

        let item_from_envs = self.item_from_envs();
        let struct_instantiation = self.instantiate_struct();

        quote! {
            #[automatically_derived]
            impl FromEnv for #struct_name {
                type Error = #error_ident;

                fn inventory() -> ::std::vec::Vec<&'static EnvItemInfo> {
                    let mut items = ::std::vec::Vec::new();
                    #(
                        #env_item_info
                    )*
                    items
                }

                fn from_env() -> ::std::result::Result<Self, FromEnvErr<Self::Error>> {
                    #(
                        #item_from_envs
                    )*

                    ::std::result::Result::Ok(#struct_instantiation)
                }
            }
        }
    }

    fn expand_mod(&self) -> TokenStream {
        // let expanded_impl = expand_impl(input);
        let expanded_error = self.expand_error();
        let expanded_impl = self.expand_impl();
        let crate_name = &self.crate_name;
        let error_ident = self.error_ident();

        let mod_ident =
            syn::parse_str::<syn::Ident>(&format!("__from_env_impls_{}", self.ident)).unwrap();

        quote! {
            pub use #mod_ident::#error_ident;
            mod #mod_ident {
                use super::*;
                use #crate_name::utils::from_env::{FromEnv, FromEnvErr, FromEnvVar, EnvItemInfo};

                #expanded_impl

                #expanded_error
            }
        }
    }
}
