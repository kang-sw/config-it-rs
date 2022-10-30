use std::borrow::Borrow;
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse2;
use super::parsing::*;

pub(self) fn generate(ty: TypeDesc) -> TokenStream {
    let identifier = ty.identifier;
    let vis = ty.type_visibility;
    let generics = ty.generics;

    let fields = (&ty.fields).iter().map(
        |x| {
            let vis = &x.visibility;
            let ident = &x.identifier;
            let ty = &x.src_type;

            quote! {

            }
        }
    );

    quote! {

    }
}


#[test]
fn test_macro() {
    let raw = r###"
        pub struct MyStruct<T, Y> {
          #[perfkit(default=34, min=0, max=154,  env="MY_ENV_VAR_NAME", no_import, no_export, hidden)]
          my_var : i32,

          pub my_var_2 : f32,

          /// My elels dsa 1
          /// My elels dsa 2
          /// My elels dsa 3
          /// My elels dsa 4
          #[perfkit()]
          pub my_var_emp : f32,

          #[perfkit(no_import)]
          pub my_var_4 : f32,

          ///
          /// Hello, world!
          ///
          #[perfkit(one_of(1,2,3,4))]
          pub my_var_3 : f64
        }
    "###;

    // println!("{}", test_input(raw.parse().unwrap()).to_string());
    let r = decompose_input(parse2(raw.parse().unwrap()).unwrap()).unwrap();
    println!("{}", generate(r));
}
