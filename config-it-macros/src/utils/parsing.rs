#![allow(dead_code)]

use std::iter::zip;

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::ToTokens;

use syn::spanned::Spanned;
use syn::Data::Struct;
use syn::{Attribute, DeriveInput, MetaNameValue};

///
///
/// Type information
///
///
pub struct TypeDesc {
    pub type_visibility: syn::Visibility,
    pub identifier: syn::Ident,
    pub generics: syn::Generics,

    pub fields: Vec<FieldDesc>,
    pub invisibles: Vec<InvisibleFieldDesc>,
}

pub struct FieldDesc {
    pub visibility: syn::Visibility,
    pub identifier: syn::Ident,
    pub src_type: syn::Type,

    pub docstring: String,
    pub alias: Option<syn::Lit>,

    pub default_value: Option<syn::Lit>,
    pub default_expr: Option<syn::Lit>,
    pub min: Option<syn::Lit>,
    pub max: Option<syn::Lit>,
    pub one_of: Option<syn::MetaList>,

    pub env_var: Option<syn::Lit>,
    pub access_level: Option<[AccessLevel; 2]>,

    pub flag_transient: bool,
    pub flag_disable_import: bool,
    pub flag_disable_export: bool,
}

pub struct InvisibleFieldDesc {
    pub identifier: syn::Ident,
    pub src_type: syn::Type,
    pub default_expr: Option<syn::Lit>,
    pub default_tokens: Option<TokenStream>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AccessLevel {
    Guest,
    User,
    Admin,
    Off,
}

///
///
/// Parses incoming derive input, then publish it as
///
pub fn decompose_input(input: DeriveInput) -> Result<TypeDesc, (Span, String)> {
    let data = if let Struct(data) = input.data {
        data
    } else {
        return Err((input.ident.span(), "Non-struct type is not permitted".into()));
    };

    // Retrieve specified namespace of config_it, since user may alias config_it module
    let mut out = TypeDesc {
        type_visibility: input.vis,
        identifier: input.ident,
        generics: input.generics,
        fields: Vec::with_capacity(data.fields.len()),
        invisibles: Vec::new(),
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
            visibility: field.vis,
            default_value: Default::default(),
            default_expr: Default::default(),
            access_level: Default::default(),
            docstring: String::with_capacity(200),
            alias: Default::default(),
            min: Default::default(),
            max: Default::default(),
            one_of: Default::default(),
            env_var: Default::default(),
            flag_transient: Default::default(),
            flag_disable_import: Default::default(),
            flag_disable_export: Default::default(),
        };

        let mut has_any_valid_attr = false;

        #[cfg(feature = "nocfg")]
        let mut invis_default_attr = None;

        for attr in field.attrs {
            #[cfg(feature = "nocfg")]
            if attr.path.is_ident("nocfg") {
                invis_default_attr = Some(attr);
                continue;
            }

            has_any_valid_attr = has_any_valid_attr | decompose_attribute(&mut desc, attr)?;
        }

        if has_any_valid_attr {
            desc.docstring.shrink_to_fit();
            out.fields.push(desc);

            continue;
        } else {
            #[allow(unused_mut)]
            let mut invis_desc = InvisibleFieldDesc {
                identifier: desc.identifier,
                src_type: desc.src_type,
                default_expr: None,
                default_tokens: None,
            };

            #[cfg(feature = "nocfg")]
            match invis_default_attr.as_ref().map(|x| x.parse_meta()) {
                Some(Ok(syn::Meta::NameValue(MetaNameValue { lit, .. }))) => {
                    invis_desc.default_expr = Some(lit);
                }

                Some(Ok(syn::Meta::List(lst))) => {
                    invis_desc.default_tokens = Some(lst.nested.to_token_stream());
                }

                Some(Err(e)) => {
                    panic!(
                        concat!(
                            "nocfg parse error at {arg:?} for error {e:?},",
                            " the original attribute was: {attr:?}"
                        ),
                        arg = invis_desc.identifier,
                        e = e,
                        attr = invis_default_attr.to_token_stream().to_string()
                    )
                }

                _ => (),
            }

            out.invisibles.push(invis_desc);
        }
    }

    Ok(out)
}

fn retrieve_namespace(attrs: Vec<Attribute>) -> Option<TokenTree> {
    let puncts: Vec<_> = attrs
        .into_iter()
        // Select 'derive' attribute
        .filter(|x| x.path.is_ident("derive"))
        .map(|x| {
            // Flatten all derivatives that are included in derive(...)
            x.tokens
                .into_iter()
                .filter_map(|x| match x {
                    TokenTree::Group(s) => Some(s),
                    _ => None,
                })
                .map(|x| x.stream())
                .flatten()
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect();

    let Some(pos) = puncts.iter().position(|x| {
            let TokenTree::Ident(id) = x else { return false};
            id == "Template"
        }) else {
            return None;
        };

    // From 'pos' trace reversely, find first ':' or ','
    let first_punct = (0..pos).rev().find_map(|idx| {
        let TokenTree::Punct(p) = &puncts[idx] else { return None };

        if p.as_char() == ':' || p.as_char() == ',' {
            Some(p)
        } else {
            None
        }
    });

    if first_punct.is_none() || first_punct.unwrap().as_char() == ',' {
        // No namespace specfied
        return None;
    }

    // It seems our 'Template' specified with namespace ...
    // Find its name.
    let comma_pos = (0..pos)
        .rev()
        .find(|idx| {
            let TokenTree::Punct(punc) = &puncts[*idx] else { return false };
            punc.as_char() == ','
        })
        .map_or(0, |x| x + 1);

    let mut puncts = puncts;
    Some(puncts.swap_remove(comma_pos))
}

fn decompose_attribute(desc: &mut FieldDesc, attr: syn::Attribute) -> Result<bool, (Span, String)> {
    // Simply ignores non-perfkit attribute
    if attr.path.is_ident("doc") {
        let Ok(NameValue(v)) = attr.parse_meta() else { return Ok(false) };
        let syn::Lit::Str(v) = &v.lit else { return Ok(false) };

        desc.docstring += &v.value();
        return Ok(false);
    }

    #[cfg(feature = "more_attr")]
    const ATTRS: [&str; 3] = ["config_it", "config", "cfg"];
    #[cfg(not(feature = "more_attr"))]
    const ATTRS: [&str; 1] = ["config_it"];

    if ATTRS.into_iter().all(|x| attr.path.is_ident(x) == false) {
        return Ok(false);
    };

    use syn::Meta::*;
    use syn::NestedMeta::*;

    let meta_list = if let Ok(List(m)) = attr.parse_meta() {
        m
    } else {
        return Ok(true);
    };

    for meta in meta_list.nested {
        match meta {
            Meta(List(v)) if v.path.is_ident("one_of") => desc.one_of = Some(v),
            Meta(List(v)) if v.path.is_ident("access") => {
                assert!(
                    desc.access_level.is_none(),
                    "'{}': cannot specify both 'hidden' and 'access'",
                    desc.identifier.to_string()
                );

                let mut read = None;
                let mut write = None;

                for (dst, item) in zip([&mut read, &mut write], v.nested) {
                    let Meta(Path(pat)) = &item else {
                        panic!("'{}': invalid access attribute {}", desc.identifier.to_string(), item.to_token_stream().to_string());
                    };

                    if pat.is_ident("off") {
                        *dst = Some(AccessLevel::Off);
                    } else if pat.is_ident("admin") {
                        *dst = Some(AccessLevel::Admin);
                    } else if pat.is_ident("user") {
                        *dst = Some(AccessLevel::User);
                    } else if pat.is_ident("guest") {
                        *dst = Some(AccessLevel::Guest);
                    } else {
                        panic!(
                            "'{}': invalid access attribute '{}'",
                            desc.identifier.to_string(),
                            item.to_token_stream().to_string()
                        );
                    }
                }

                if write.is_none() {
                    write = read;
                }

                if read.unwrap() > write.unwrap() {
                    panic!(
                        "'{}': read access level must be less than or equal to write access level",
                        desc.identifier.to_string()
                    );
                }

                desc.access_level = Some([read.expect("must set valid value"), write.unwrap()]);
            }

            Meta(NameValue(MetaNameValue { path, lit, .. })) => {
                let dst = if path.is_ident("min") {
                    &mut desc.min
                } else if path.is_ident("max") {
                    &mut desc.max
                } else if path.is_ident("default_expr") {
                    &mut desc.default_expr
                } else if path.is_ident("default") {
                    &mut desc.default_value
                } else if path.is_ident("env") {
                    &mut desc.env_var
                } else if path.is_ident("alias") {
                    let syn::Lit::Str(x) = &lit else {
                        panic!("'{}' non-string alias!", desc.identifier.to_string());
                    };

                    assert!(!x.value().contains(&['^', '~']), "^ and ~ is special delimiter");
                    &mut desc.alias
                } else {
                    return Err((attr.span(), "Unknonw attribute".to_string()));
                };

                *dst = Some(lit);
            }

            Meta(Path(v)) if v.is_ident("no_export") => desc.flag_disable_export = true,
            Meta(Path(v)) if v.is_ident("no_import") => desc.flag_disable_import = true,

            Meta(Path(v)) if v.is_ident("hidden") => {
                assert!(
                    desc.access_level.is_none(),
                    "'{}': cannot specify both 'hidden' and 'access'",
                    desc.identifier.to_string()
                );

                desc.access_level = Some([AccessLevel::Off; 2]);
            }

            Meta(Path(v)) if v.is_ident("transient") => {
                desc.flag_disable_import = true;
                desc.flag_disable_export = true
            }

            _ => {
                panic!(
                    "invalid attribute for '{}' -> '{}'",
                    desc.identifier.to_string(),
                    meta.to_token_stream().to_string()
                )
            }
        }
    }

    Ok(true)
}

///
///
///
/// Concept test below
///
///
#[cfg(any())]
#[cfg(feature = "is_proc_macro_impl")]
use proc_macro2::TokenStream;

#[cfg(any())]
#[cfg(feature = "is_proc_macro_impl")]
fn test_input(input: TokenStream) -> TokenStream {
    use syn::{parse2, parse_macro_input, AttrStyle};
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
                use syn::Meta::*;
                use syn::NestedMeta::*;

                if let Ok(List(meta)) = x.parse_meta() {
                    meta.nested.into_iter().for_each(|meta| match &meta {
                        Meta(NameValue(v)) => {
                            println!(
                                "      META NAMEVAL: PATH({}) LIT({})",
                                v.path.to_token_stream().to_string(),
                                v.lit.to_token_stream().to_string()
                            );
                        }
                        Meta(Path(v)) => {
                            println!("      META PATH ({})", v.to_token_stream().to_string());
                        }
                        Meta(List(v)) => {
                            println!(
                                "      META LIST ({}) -> {}",
                                v.path.to_token_stream().to_string(),
                                v.nested.to_token_stream().to_string()
                            );
                        }
                        Lit(v) => {
                            println!("      META LIT ({})", v.to_token_stream().to_string());
                        }
                    })
                }
                println!(
                    "    {}",
                    (if let AttrStyle::Inner(b) = x.style {
                        b.to_token_stream().to_string() + "Inner"
                    } else {
                        "Outer".to_string()
                    })
                );
            }
        }
    }

    TokenStream::new()
}
