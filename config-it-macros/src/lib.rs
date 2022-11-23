use core::panic;

use proc_macro::TokenStream;
use quote::{quote_spanned, quote};
use syn::DeriveInput;

mod utils;

///
/// Generates required properties for config set properties
/// 
#[proc_macro_derive(ConfigGroupData, attributes(config_it))]
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
    
    let generated = match utils::generate(parse_result).into() {
        Ok(r) => r,
        Err((span, str)) => quote_spanned!(
                span => compile_error!(#str)
            )
    };
    
    let generated = generated.to_string();
    
    quote!{
        fn hello() -> &'static str {
            #generated
        }
    }.into()
}
