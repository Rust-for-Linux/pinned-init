// SPDX-License-Identifier: GPL-2.0

use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};

pub(crate) fn pinned_drop(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut toks = input.into_iter().collect::<Vec<_>>();
    assert!(!toks.is_empty());
    // ensure that we have an impl item
    assert!(matches!(&toks[0], TokenTree::Ident(i) if i.to_string() == "impl"));
    // ensure that we are implementing `PinnedDrop`
    let mut nesting: usize = 0;
    let mut pinned_drop_idx = None;
    for (i, tt) in toks.iter().enumerate() {
        match tt {
            TokenTree::Punct(p) if p.as_char() == '<' => {
                nesting += 1;
            }
            TokenTree::Punct(p) if p.as_char() == '>' => {
                nesting = nesting.checked_sub(1).unwrap();
                continue;
            }
            _ => {}
        }
        if i >= 1 && nesting == 0 {
            assert!(
                matches!(tt, TokenTree::Ident(i) if i.to_string() == "PinnedDrop"),
                "expected 'PinnedDrop', found: '{:?}'",
                tt
            );
            pinned_drop_idx = Some(i);
            break;
        }
    }
    let idx = pinned_drop_idx.unwrap();
    //inserting `::pinned_init::` in reverse order
    toks.insert(idx, TokenTree::Punct(Punct::new(':', Spacing::Alone)));
    toks.insert(idx, TokenTree::Punct(Punct::new(':', Spacing::Joint)));
    toks.insert(
        idx,
        TokenTree::Ident(Ident::new("pinned_init", Span::call_site())),
    );
    toks.insert(idx, TokenTree::Punct(Punct::new(':', Spacing::Alone)));
    toks.insert(idx, TokenTree::Punct(Punct::new(':', Spacing::Joint)));
    if let Some(TokenTree::Group(last)) = toks.pop() {
        let mut inner = last.stream().into_iter().collect::<Vec<_>>();
        if let Some(TokenTree::Group(inner_last)) = inner.pop() {
            // make the impl unsafe
            toks.insert(0, TokenTree::Ident(Ident::new("unsafe", Span::call_site())));
            // make the first function unsafe
            inner.insert(0, TokenTree::Ident(Ident::new("unsafe", Span::call_site())));
            // re-add the body
            inner.push(TokenTree::Group(inner_last.clone()));
            add_ensure_no_unsafe_op_in_drop(&mut inner, inner_last);
            toks.push(TokenTree::Group(Group::new(
                Delimiter::Brace,
                TokenStream::from_iter(inner),
            )));
            TokenStream::from_iter(toks)
        } else {
            toks.push(TokenTree::Group(last));
            TokenStream::from_iter(toks)
        }
    } else {
        TokenStream::from_iter(toks)
    }
}

fn add_ensure_no_unsafe_op_in_drop(v: &mut Vec<TokenTree>, inner_last: Group) {
    v.push(TokenTree::Ident(Ident::new("fn", Span::call_site())));
    v.push(TokenTree::Ident(Ident::new(
        "__ensure_no_unsafe_op_in_drop",
        Span::call_site(),
    )));
    v.push(TokenTree::Group(Group::new(
        Delimiter::Parenthesis,
        TokenStream::from_iter(vec![
            TokenTree::Ident(Ident::new("self", Span::call_site())),
            TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            TokenTree::Punct(Punct::new(':', Spacing::Joint)),
            TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            TokenTree::Ident(Ident::new("core", Span::call_site())),
            TokenTree::Punct(Punct::new(':', Spacing::Joint)),
            TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            TokenTree::Ident(Ident::new("pin", Span::call_site())),
            TokenTree::Punct(Punct::new(':', Spacing::Joint)),
            TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            TokenTree::Ident(Ident::new("Pin", Span::call_site())),
            TokenTree::Punct(Punct::new('<', Spacing::Alone)),
            TokenTree::Punct(Punct::new('&', Spacing::Alone)),
            TokenTree::Ident(Ident::new("mut", Span::call_site())),
            TokenTree::Ident(Ident::new("Self", Span::call_site())),
            TokenTree::Punct(Punct::new('>', Spacing::Alone)),
        ]),
    )));
    v.push(TokenTree::Group(Group::new(
        Delimiter::Brace,
        TokenStream::from_iter(vec![
            TokenTree::Ident(Ident::new("if", Span::call_site())),
            TokenTree::Ident(Ident::new("false", Span::call_site())),
            TokenTree::Group(inner_last),
        ]),
    )));
}
