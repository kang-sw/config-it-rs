use proc_macro2 as proc_macro;
use proc_macro::{TokenStream, TokenTree};
use quote::{quote, ToTokens};
use itertools::{Itertools, Zip};

use syn::{parse2, DeriveInput, parse_macro_input, AttrStyle};
use syn::Data::Struct;
use syn::spanned::Spanned;

fn config_collection(input: TokenStream) -> TokenStream {
    let i: DeriveInput = parse2(input).unwrap();

    println!("-- 0: {}", i.ident.to_string());
    println!("-- 1: {}", i.generics.params.to_token_stream().to_string());
    println!("-- 2: {}", i.vis.to_token_stream().to_string());
    println!("-- 3: FIELDS");

    if let Struct(v) = i.data {
        let fields = v.fields;
        for (i, f) in (0..fields.len()).zip(fields.iter()) {
            let ty = f.ty.to_token_stream().to_string();
            let id = if let Some(s) = &f.ident { s.to_string() } else { "<NO_IDENT>".into() };
            let vis = f.vis.to_token_stream().to_string();

            println!("  LN {}: {} {} : {}", i, vis, id, ty);
            for x in &f.attrs {
                println!("    PATH: {}", x.path.to_token_stream().to_string());
                println!("    TOK: {}", x.tokens.to_token_stream().to_string());
                println!("    {}", (if let AttrStyle::Inner(b) = x.style { b.to_token_stream().to_string() + "Inner" } else { "Outer".to_string() }));
            }
        }
    }

    TokenStream::new()
}

#[test]
fn test_macro() {
    let raw = r###"
        struct MyStruct<T, Y> {
          #[perfkit(min=0, max=154)]
          my_var : i32,

          pub my_var_2 : f32,

          ///
          /// Hello, world!
          ///
          #[perfkit(one_of=[])]
          pub my_var_3 : f64
        }
    "###;

    println!("{}", config_collection(raw.parse().unwrap()).to_string());
}
