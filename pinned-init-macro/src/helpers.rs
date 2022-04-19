use proc_macro2::*;
use quote::*;
use std::collections::*;
use syn::{parse::*, *};

pub fn has_outer_attr<'a>(attrs: impl IntoIterator<Item = &'a Attribute>, name: &str) -> bool {
    attrs
        .into_iter()
        .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident(name))
}

/// splits the given generics into impl_generics, type_generics and the where
/// clause, similar to [`Generics::split_for_impl`], but does not produce the '<' and '>' tokens,
/// in order to allow generic extensions
pub fn my_split_for_impl(generics: &Generics) -> (TokenStream, TokenStream, Option<&WhereClause>) {
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let mut impl_generics = quote! { #impl_generics }
        .into_iter()
        .collect::<VecDeque<_>>();
    let mut type_generics = quote! { #type_generics }
        .into_iter()
        .collect::<VecDeque<_>>();
    macro_rules! pop {
        (front $e:expr, $c:literal) => {{
            let pop = $e.pop_front();
            if let Some(TokenTree::Punct(p)) = &pop {
                assert_eq!(p.as_char(), $c);
            } else {
                panic!(
                    "invalid internal state, expected {}, but found {:?}",
                    $c, pop
                );
            }
        }};
        (back $e:expr, $c:literal) => {{
            let pop = $e.pop_back();
            if let Some(TokenTree::Punct(p)) = &pop {
                assert_eq!(p.as_char(), $c);
            } else {
                panic!(
                    "invalid internal state, expected {}, but found {:?}",
                    $c, pop
                );
            }
        }};
    }
    if !impl_generics.is_empty() {
        pop!(front impl_generics, '<');
        pop!(back impl_generics, '>');
    }
    if !type_generics.is_empty() {
        pop!(front type_generics, '<');
        pop!(back type_generics, '>');
    }
    (
        impl_generics.into_iter().collect(),
        type_generics.into_iter().collect(),
        where_clause,
    )
}

pub enum ManualInitParam {
    /// Implements BeginPinnedInit instead of BeginInit
    Pinned,
    /// Delegates the token stream to the pin_project invokation, only makes sense if also
    /// ManualInitParam::Pinned
    PinProject(TokenStream),
}

pub fn parse_attrs(stream: ParseStream) -> syn::parse::Result<Vec<ManualInitParam>> {
    let mut res = vec![];
    loop {
        res.push(
            match stream.step(|cursor| {
                if cursor.eof() {
                    Ok((None, *cursor))
                } else {
                    match cursor.ident() {
                        Some((ident, next)) => match format!("{ident}").as_ref() {
                            "pinned" => Ok((Some(ManualInitParam::Pinned), next)),
                            "pin_project" => {
                                if let Some((inner, _, next)) = next.group(Delimiter::Parenthesis) {
                                    Ok((
                                        Some(ManualInitParam::PinProject(inner.token_stream())),
                                        next,
                                    ))
                                } else {
                                    Err(cursor.error("Expected `pin_project(<args>)`."))
                                }
                            }
                            _ => Err(cursor.error("Expected `pinned` or `pin_project(<args>)`.")),
                        },
                        _ => Err(cursor.error("Expected `pinned` or `pin_project(<args>)`.")),
                    }
                }
            })? {
                Some(val) => val,
                None => return Ok(res),
            },
        );
        stream.step(|cursor| {
            if !cursor.eof() {
                match cursor.punct() {
                    Some((punct, next)) if punct.as_char() == ',' => Ok(((), next)),
                    _ => Err(cursor.error("Expected ','")),
                }
            } else {
                Ok(((), *cursor))
            }
        })?;
    }
}
