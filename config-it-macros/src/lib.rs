use core::panic;

use proc_macro::TokenStream;
use quote::quote_spanned;
use syn::DeriveInput;

///
/// Generates required properties for config set properties
///
#[cfg(all(not(feature = "more_attr"), not(feature = "nocfg")))]
#[proc_macro_derive(Template, attributes(config_it))]
pub fn derive_collect_fn(item: TokenStream) -> TokenStream {
    derive_collect_fn_impl(item)
}

#[cfg(all(not(feature = "more_attr"), feature = "nocfg"))]
#[proc_macro_derive(Template, attributes(config_it, nocfg))]
pub fn derive_collect_fn(item: TokenStream) -> TokenStream {
    derive_collect_fn_impl(item)
}

#[cfg(all(feature = "more_attr", not(feature = "nocfg")))]
#[proc_macro_derive(Template, attributes(config_it, config))]
pub fn derive_collect_fn(item: TokenStream) -> TokenStream {
    derive_collect_fn_impl(item)
}

#[cfg(all(feature = "more_attr", feature = "nocfg"))]
#[proc_macro_derive(Template, attributes(config_it, nocfg, config))]
pub fn derive_collect_fn(item: TokenStream) -> TokenStream {
    derive_collect_fn_impl(item)
}

fn derive_collect_fn_impl(item: TokenStream) -> TokenStream {
    Default::default()
}
