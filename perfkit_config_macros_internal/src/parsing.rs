use proc_macro2 as proc_macro;
use proc_macro::{TokenStream, TokenTree};
use proc_macro2::Span;
use quote::{ToTokens};

use syn::{parse2, DeriveInput, parse_macro_input, AttrStyle, MetaNameValue};
use syn::Data::Struct;
use syn::spanned::Spanned;

///
///
/// Type information
///
pub(self) struct TypeDesc {
    pub type_visibility: syn::Visibility,
    pub identifier: syn::Ident,
    pub generics: syn::Generics,

    pub fields: Vec<FieldDesc>,
}

pub(self) struct FieldDesc {
    pub identifier: syn::Ident,
    pub src_type: syn::Type,

    pub docstring: String,

    pub default_value: Option<syn::Lit>,
    pub min: Option<syn::Lit>,
    pub max: Option<syn::Lit>,
    pub one_of: Option<syn::MetaList>,

    pub env_var: Option<syn::Lit>,

    pub flag_transient: bool,
    pub flag_disable_import: bool,
    pub flag_disable_export: bool,
    pub flag_hidden: bool,
}

///
///
/// Parses incoming derive input, then publish it as
///
pub(self) fn decompose_input(input: DeriveInput) -> Result<TypeDesc, (Span, String)> {
    let data = if let Struct(data) = input.data {
        data
    } else {
        return Err((input.ident.span(), "Non-struct type is not permitted".into()));
    };

    let mut out = TypeDesc {
        type_visibility: input.vis,
        identifier: input.ident,
        generics: input.generics,
        fields: Vec::with_capacity(data.fields.len()),
    };

    for field in data.fields {
        let identifier = if let Some(ident) = field.ident {
            ident
        } else {
            return Err((field.span(), "Identifier must exist".into()));
        };

        let mut desc = FieldDesc {
            identifier,
            src_type: field.ty,
            default_value: Default::default(),
            docstring: String::with_capacity(200),
            min: Default::default(),
            max: Default::default(),
            one_of: Default::default(),
            env_var: Default::default(),
            flag_transient: Default::default(),
            flag_disable_import: Default::default(),
            flag_disable_export: Default::default(),
            flag_hidden: Default::default(),
        };

        let mut has_any_valid_attr = false;
        for attr in field.attrs {
            has_any_valid_attr = has_any_valid_attr | decompose_attribute(&mut desc, attr);
        }

        if has_any_valid_attr {
            desc.docstring.shrink_to_fit();
            out.fields.push(desc);
        }
    }

    Ok(out)
}

fn decompose_attribute(desc: &mut FieldDesc, attr: syn::Attribute) -> bool {
    // Simply ignores non-perfkit attribute
    if false == attr.path.is_ident("perfkit") { return false; };

    use syn::NestedMeta::*;
    use syn::Meta::*;

    let meta_list = if let Ok(List(m)) = attr.parse_meta() { m } else { return false; };
    meta_list.nested.into_iter().for_each(|meta| {
        match meta {
            Meta(List(v)) if v.path.is_ident("one_of") => { desc.one_of = Some(v) }

            Meta(NameValue(MetaNameValue { path, lit, .. })) => {
                if path.is_ident("min") {
                    desc.min = Some(lit);
                } else if path.is_ident("max") {
                    desc.max = Some(lit);
                } else if path.is_ident("env") {
                    desc.env_var = Some(lit);
                }
            }

            Meta(Path(v)) if v.is_ident("no_export") => { desc.flag_disable_export = true }
            Meta(Path(v)) if v.is_ident("no_import") => { desc.flag_disable_import = true }
            Meta(Path(v)) if v.is_ident("hidden") => { desc.flag_hidden = true }

            Meta(Path(v)) if v.is_ident("transient") => {
                desc.flag_disable_import = true;
                desc.flag_disable_export = true
            }

            _ => {}
        }
    });

    true
}


///
///
///
/// Concept test below
///
#[cfg(test)]
fn test_input(input: TokenStream) -> TokenStream {
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
                x.path.span();

                println!("    PATH: {}", x.path.to_token_stream().to_string());
                println!("    TOK: {}", x.tokens.to_token_stream().to_string());
                use syn::NestedMeta::*;
                use syn::Meta::*;

                if let Ok(List(meta)) = x.parse_meta() {
                    meta.nested.into_iter()
                        .for_each(
                            |meta| {
                                match &meta {
                                    Meta(NameValue(v)) => {
                                        println!("      META NAMEVAL: PATH({}) LIT({})", v.path.to_token_stream().to_string(), v.lit.to_token_stream().to_string());
                                    }
                                    Meta(Path(v)) => {
                                        println!("      META PATH ({})", v.to_token_stream().to_string());
                                    }
                                    Meta(List(v)) => {
                                        println!("      META LIST ({}) -> {}", v.path.to_token_stream().to_string(), v.nested.to_token_stream().to_string());
                                    }
                                    Lit(v) => {
                                        println!("      META LIT ({})", v.to_token_stream().to_string());
                                    }
                                }
                            }
                        )
                }
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

    r;
}
