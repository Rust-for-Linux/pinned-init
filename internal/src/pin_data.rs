// SPDX-License-Identifier: Apache-2.0 OR MIT

use proc_macro2::{Group, Punct, Spacing, TokenStream, TokenTree};
use quote::quote;

pub(crate) struct Generics {
    pub(crate) decl_generics: Vec<TokenTree>,
    pub(crate) impl_generics: Vec<TokenTree>,
    pub(crate) ty_generics: Vec<TokenTree>,
}

/// Parses the given `TokenStream` into `Generics` and the rest.
///
/// The generics are not present in the rest, but a where clause might remain.
pub(crate) fn parse_generics(input: TokenStream) -> (Generics, Vec<TokenTree>) {
    // The generics with bounds and default values.
    let mut decl_generics = vec![];
    // `impl_generics`, the declared generics with their bounds.
    let mut impl_generics = vec![];
    // Only the names of the generics, without any bounds.
    let mut ty_generics = vec![];
    // Tokens not related to the generics e.g. the `where` token and definition.
    let mut rest = vec![];
    // The current level of `<`.
    let mut nesting = 0;
    let mut toks = input.into_iter();
    // If we are at the beginning of a generic parameter.
    let mut at_start = true;
    let mut skip_until_comma = false;
    while let Some(tt) = toks.next() {
        if nesting == 1 && matches!(&tt, TokenTree::Punct(p) if p.as_char() == '>') {
            // Found the end of the generics.
            break;
        } else if nesting >= 1 {
            decl_generics.push(tt.clone());
        }
        match tt.clone() {
            TokenTree::Punct(p) if p.as_char() == '<' => {
                if nesting >= 1 && !skip_until_comma {
                    // This is inside of the generics and part of some bound.
                    impl_generics.push(tt);
                }
                nesting += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '>' => {
                // This is a parsing error, so we just end it here.
                if nesting == 0 {
                    break;
                } else {
                    nesting -= 1;
                    if nesting >= 1 && !skip_until_comma {
                        // We are still inside of the generics and part of some bound.
                        impl_generics.push(tt);
                    }
                }
            }
            TokenTree::Punct(p) if skip_until_comma && p.as_char() == ',' => {
                if nesting == 1 {
                    impl_generics.push(tt.clone());
                    impl_generics.push(tt);
                    skip_until_comma = false;
                }
            }
            _ if !skip_until_comma => {
                match nesting {
                    // If we haven't entered the generics yet, we still want to keep these tokens.
                    0 => rest.push(tt),
                    1 => {
                        // Here depending on the token, it might be a generic variable name.
                        match tt.clone() {
                            TokenTree::Ident(i) if at_start && i.to_string() == "const" => {
                                let Some(name) = toks.next() else {
                                    // Parsing error.
                                    break;
                                };
                                impl_generics.push(tt);
                                impl_generics.push(name.clone());
                                ty_generics.push(name.clone());
                                decl_generics.push(name);
                                at_start = false;
                            }
                            TokenTree::Ident(_) if at_start => {
                                impl_generics.push(tt.clone());
                                ty_generics.push(tt);
                                at_start = false;
                            }
                            TokenTree::Punct(p) if p.as_char() == ',' => {
                                impl_generics.push(tt.clone());
                                ty_generics.push(tt);
                                at_start = true;
                            }
                            // Lifetimes begin with `'`.
                            TokenTree::Punct(p) if p.as_char() == '\'' && at_start => {
                                impl_generics.push(tt.clone());
                                ty_generics.push(tt);
                            }
                            // Generics can have default values, we skip these.
                            TokenTree::Punct(p) if p.as_char() == '=' => {
                                skip_until_comma = true;
                            }
                            _ => impl_generics.push(tt),
                        }
                    }
                    _ => impl_generics.push(tt),
                }
            }
            _ => {}
        }
    }
    rest.extend(toks);
    (
        Generics {
            impl_generics,
            decl_generics,
            ty_generics,
        },
        rest,
    )
}

pub(crate) fn pin_data(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let args: TokenStream = args.into();
    // This proc-macro only does some pre-parsing and then delegates the actual parsing to
    // `pin_init::__pin_data!`.

    let (
        Generics {
            decl_generics,
            impl_generics,
            ty_generics,
        },
        rest,
    ) = parse_generics(input.into());
    // The struct definition might contain the `Self` type. Since `__pin_data!` will define a new
    // type with the same generics and bounds, this poses a problem, since `Self` will refer to the
    // new type as opposed to this struct definition. Therefore we have to replace `Self` with the
    // concrete name.

    // Errors that occur when replacing `Self` with `struct_name`.
    let mut errs = TokenStream::new();
    // The name of the struct with ty_generics.
    let struct_name = rest
        .iter()
        .skip_while(|tt| !matches!(tt, TokenTree::Ident(i) if i.to_string() == "struct"))
        .nth(1)
        .and_then(|tt| match tt {
            TokenTree::Ident(_) => {
                let tt = tt.clone();
                let mut res = vec![tt];
                if !ty_generics.is_empty() {
                    // We add this, so it is maximally compatible with e.g. `Self::CONST` which
                    // will be replaced by `StructName::<$generics>::CONST`.
                    res.push(TokenTree::Punct(Punct::new(':', Spacing::Joint)));
                    res.push(TokenTree::Punct(Punct::new(':', Spacing::Alone)));
                    res.push(TokenTree::Punct(Punct::new('<', Spacing::Alone)));
                    res.extend(ty_generics.iter().cloned());
                    res.push(TokenTree::Punct(Punct::new('>', Spacing::Alone)));
                }
                Some(res)
            }
            _ => None,
        })
        .unwrap_or_else(|| {
            // If we did not find the name of the struct then we will use `Self` as the replacement
            // and add a compile error to ensure it does not compile.
            errs.extend(
                "::core::compile_error!(\"Could not locate type name.\");"
                    .parse::<TokenStream>()
                    .unwrap(),
            );
            "Self".parse::<TokenStream>().unwrap().into_iter().collect()
        });
    let impl_generics = impl_generics
        .into_iter()
        .flat_map(|tt| replace_self_and_deny_type_defs(&struct_name, tt, &mut errs))
        .collect::<Vec<_>>();
    let mut rest = rest
        .into_iter()
        .flat_map(|tt| {
            // We ignore top level `struct` tokens, since they would emit a compile error.
            if matches!(&tt, TokenTree::Ident(i) if i.to_string() == "struct") {
                vec![tt]
            } else {
                replace_self_and_deny_type_defs(&struct_name, tt, &mut errs)
            }
        })
        .collect::<Vec<_>>();
    // This should be the body of the struct `{...}`.
    let last = rest.pop();
    let mut quoted = quote!(::pin_init::__pin_data! {
        parse_input:
        @args(#args),
        @sig(#(#rest)*),
        @impl_generics(#(#impl_generics)*),
        @ty_generics(#(#ty_generics)*),
        @decl_generics(#(#decl_generics)*),
        @body(#last),
    });
    quoted.extend(errs);
    quoted.into()
}

/// Replaces `Self` with `struct_name` and errors on `enum`, `trait`, `struct` `union` and `impl`
/// keywords.
///
/// The error is appended to `errs` to allow normal parsing to continue.
fn replace_self_and_deny_type_defs(
    struct_name: &Vec<TokenTree>,
    tt: TokenTree,
    errs: &mut TokenStream,
) -> Vec<TokenTree> {
    match tt {
        TokenTree::Ident(ref i)
            if i.to_string() == "enum"
                || i.to_string() == "trait"
                || i.to_string() == "struct"
                || i.to_string() == "union"
                || i.to_string() == "impl" =>
        {
            errs.extend(
                format!(
                    "::core::compile_error!(\"Cannot use `{i}` inside of struct definition with \
                        `#[pin_data]`.\");"
                )
                .parse::<TokenStream>()
                .unwrap()
                .into_iter()
                .map(|mut tok| {
                    tok.set_span(tt.span());
                    tok
                }),
            );
            vec![tt]
        }
        TokenTree::Ident(i) if i.to_string() == "Self" => struct_name.clone(),
        TokenTree::Literal(_) | TokenTree::Punct(_) | TokenTree::Ident(_) => vec![tt],
        TokenTree::Group(g) => vec![TokenTree::Group(Group::new(
            g.delimiter(),
            g.stream()
                .into_iter()
                .flat_map(|tt| replace_self_and_deny_type_defs(struct_name, tt, errs))
                .collect(),
        ))],
    }
}
