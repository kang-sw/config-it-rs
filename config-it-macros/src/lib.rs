use core::panic;

use proc_macro::TokenStream;
use quote::quote_spanned;
use syn::DeriveInput;

mod utils;

///
/// Generates required properties for config set properties
/// 
#[proc_macro_derive(CollectPropMeta)]
pub fn derive_collect_fn(item: TokenStream) -> TokenStream {
    let Ok(result) = syn::parse::<DeriveInput>(item) else { 
        panic!("Failed to parse syntax!")
    };

    let parse_result = match utils::parsing::decompose_input(result) {
        Ok(r) => r ,
        Err((span, str)) => {
            return quote_spanned!(
                span => compile_error!(#str)
            ).into();
        }
    };
    
    match utils::generate(parse_result).into() {
        Ok(r) => r,
        Err((span, str)) => quote_spanned!(
                span => compile_error!(#str)
            )
    }.into()
}
