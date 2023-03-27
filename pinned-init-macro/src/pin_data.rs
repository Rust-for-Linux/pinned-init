// SPDX-License-Identifier: Apache-2.0 OR MIT

use proc_macro2::{Punct, Spacing, TokenStream, TokenTree};

pub(crate) fn pin_data(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let args: TokenStream = args.into();
    let input: TokenStream = input.into();
    // This proc-macro only does some pre-parsing and then delegates the actual parsing to
    // `pinned_init::__pin_data!`.
    //
    // In here we only collect the generics, since parsing them in declarative macros is very
    // elaborate. We also do not need to analyse their structure, we only need to collect them.

    // `impl_generics`, the declared generics with their bounds.
    let mut impl_generics = vec![];
    // Only the names of the generics, without any bounds.
    let mut ty_generics = vec![];
    // Tokens not related to the generics e.g. the `impl` token.
    let mut rest = vec![];
    // The current level of `<`.
    let mut nesting = 0;
    let mut toks = input.into_iter();
    // if we are at the beginning of a generic parameter
    let mut at_start = true;
    for tt in &mut toks {
        match tt.clone() {
            TokenTree::Punct(p) if p.as_char() == '<' => {
                if nesting >= 1 {
                    impl_generics.push(tt);
                }
                nesting += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '>' => {
                if nesting == 0 {
                    break;
                } else {
                    nesting -= 1;
                    if nesting >= 1 {
                        impl_generics.push(tt);
                    }
                    if nesting == 0 {
                        break;
                    }
                }
            }
            tt => {
                if nesting == 1 {
                    match &tt {
                        TokenTree::Ident(i) if *i == "const" => {}
                        TokenTree::Ident(_) if at_start => {
                            ty_generics.push(tt.clone());
                            ty_generics.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
                            at_start = false;
                        }
                        TokenTree::Punct(p) if p.as_char() == ',' => at_start = true,
                        TokenTree::Punct(p) if p.as_char() == '\'' && at_start => {
                            ty_generics.push(tt.clone());
                        }
                        _ => {}
                    }
                }
                if nesting >= 1 {
                    impl_generics.push(tt);
                } else if nesting == 0 {
                    rest.push(tt);
                }
            }
        }
    }
    rest.extend(toks);
    // This should be the body of the struct `{...}`.
    let last = rest.pop();
    quote::quote!(::pinned_init::__pin_data! {
        parse_input:
        @args(#args),
        @sig(#(#rest)*),
        @impl_generics(#(#impl_generics)*),
        @ty_generics(#(#ty_generics)*),
        @body(#last),
    })
    .into()
}
