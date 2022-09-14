#![feature(proc_macro_quote)]
use proc_macro::*;
use std::collections::*;

macro_rules! convert {
    ($($into_iter:ident),*) => {
        $(let $into_iter = TokenStream::from_iter($into_iter);)*
    }
}

#[proc_macro_attribute]
pub fn init(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut tokens = item.into_iter().collect::<VecDeque<_>>();
    let mut ty = vec![];
    let mut body = loop {
        let next = tokens
            .pop_front()
            .unwrap_or_else(|| panic!("could not locate initializer body! {ty:?}"));
        match next {
            TokenTree::Group(g)
                if matches!(g.delimiter(), Delimiter::Brace)
                    && (tokens.is_empty()
                        || (tokens.len() == 1
                            && matches!(&tokens[0], TokenTree::Punct(p) if p.as_char()==',' && p.spacing()==Spacing::Alone))) =>
            {
                break g.stream().into_iter();
            }
            r => ty.push(r),
        }
    };

    let mut fields = vec![];
    while let Some(field) = parse_field_init(&mut body) {
        fields.push(field);
    }
    let mut inner = vec![];
    let mut check = vec![];
    for Field { ident, expr } in fields.iter().cloned() {
        let ident = TokenTree::Ident(ident);
        convert!(expr);
        inner.extend(quote! {
            let $ident = $expr;
            unsafe { <_ as ::simple_safe_init::Place>::init(::core::ptr::addr_of_mut!((*place).$ident), $ident)? };
        });
        check.extend(quote! {
            $ident: ::core::todo!(),
        });
    }
    convert!(ty, check);
    inner.extend(quote! {
        if false {
            #[allow(unreachable_code)]
            let _: $ty = $ty {
                $check
            };
        }
    });

    convert!(inner);
    quote! {{
        let init = move |place: *mut $ty| {
            $inner
            Ok(())
        };
        unsafe { ::simple_safe_init::Init::from_closure(init) }
    }}
}

#[derive(Debug, Clone)]
struct Field {
    ident: Ident,
    expr: Vec<TokenTree>,
}

fn parse_field_init(toks: &mut impl Iterator<Item = TokenTree>) -> Option<Field> {
    let ident = match toks.next() {
        Some(TokenTree::Ident(ident)) => ident,
        None => return None,
        Some(r) => panic!("expected an identifier, found '{r:?}'"),
    };
    match toks.next() {
        Some(TokenTree::Punct(punct))
            if punct.as_char() == ':' && punct.spacing() == Spacing::Alone => {}
        r => panic!("expected ':', found '{r:?}'"),
    }
    let mut expr = vec![];
    loop {
        match toks.next() {
            Some(TokenTree::Punct(punct))
                if punct.as_char() == ',' && punct.spacing() == Spacing::Alone =>
            {
                break;
            }
            None => break,
            Some(tt) => expr.push(tt),
        }
    }
    Some(Field { ident, expr })
}
