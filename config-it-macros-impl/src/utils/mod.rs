pub mod parsing;

use parsing::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use std::borrow::Borrow;
use syn::parse2;

pub fn generate(ty: TypeDesc) -> Result<TokenStream, (Span, String)> {
    let identifier = ty.identifier;
    let vis = ty.type_visibility;
    let generics = ty.generics;

    let fields = (&ty.fields).iter().map(|x| {
        let vis = &x.visibility;
        let ident = &x.identifier;
        let ty = &x.src_type;

        quote! {}
    });

    Ok(quote! {
        fn hello_world() {
            println!("hello, world!")
        }
    })
}
