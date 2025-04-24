use heck::ToPascalCase;
use proc_macro::TokenStream as Ts;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input, spanned::Spanned};

mod field;
use field::Field;

#[proc_macro_derive(FromEnv, attributes(from_env_var))]
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

    if matches!(data.fields, syn::Fields::Unit) {
        syn::Error::new(
            input.ident.span(),
            "FromEnv can only be derived for structs with fields",
        )
        .to_compile_error();
    }

    expand_mod(&input).into()
}

fn expand_mod(input: &syn::DeriveInput) -> TokenStream {
    let expanded_impl = expand_struct(input);
    let expanded_error = expand_error(input);

    quote! {
        #[automatically_derived]
        const _: () = {
            use ::init4_bin_base::utils::from_env::{FromEnv, FromEnvErr, FromEnvVar};

            #expanded_impl

            #expanded_error
        };
    }
}

fn expand_struct(input: &syn::DeriveInput) -> TokenStream {
    let struct_name = &input.ident;

    quote! {

        // #[automatically_derived]
        // impl FromEnv for #struct_name {

        // }
    }
}

fn error_ident(input: &syn::DeriveInput) -> syn::Ident {
    let error_name = format!("{}Error", input.ident);
    syn::parse_str::<syn::Ident>(&error_name)
        .map_err(|_| {
            syn::Error::new(input.ident.span(), "Failed to parse error ident").to_compile_error()
        })
        .unwrap()
}

fn expand_error(input: &syn::DeriveInput) -> TokenStream {
    let error_ident = error_ident(input);

    let syn::Data::Struct(data) = &input.data else {
        unreachable!()
    };
    let fields = match &data.fields {
        syn::Fields::Named(fields) => fields.named.iter().map(Field::from).collect::<Vec<_>>(),
        syn::Fields::Unnamed(fields) => fields.unnamed.iter().map(Field::from).collect::<Vec<_>>(),
        syn::Fields::Unit => unreachable!(),
    };

    let error_variants = fields
        .iter()
        .enumerate()
        .map(|(idx, field)| field.expand_enum_variant(idx))
        .collect::<Vec<_>>();

    let variant_names = fields
        .iter()
        .enumerate()
        .map(|(idx, field)| field.enum_variant_name(idx))
        .collect::<Vec<_>>();

    let s = quote! {
        #[doc("Generated error type for `FromEnv`")]
        #[derive(Debug, PartialEq, Eq)]
        pub enum #error_ident {
            #(#error_variants),*
        }

        impl ::core::error::Error for #error_ident {
            fn source(&self) -> Option<&(dyn ::core::any::Any + ::core::marker::Send + 'static)> {
                match self {
                    #(
                        Self::#variant_names(err) => Some(err),
                    )*
                }
            }

            fn description(&self) -> &str {
                match self {
                    #(
                        Self::#variant_names(err) => err.description(),
                    )*
                }
            }
        }
    };
    eprintln!("{s}");
    s
}
