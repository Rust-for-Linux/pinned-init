#![feature(proc_macro_quote)]
use proc_macro::*;
use std::collections::*;

macro_rules! convert {
    ($($into_iter:ident),*) => {
        $(let $into_iter = TokenStream::from_iter($into_iter);)*
    }
}

#[cfg(feature = "attr")]
#[proc_macro_attribute]
pub fn init_attr(_attr: TokenStream, item: TokenStream) -> TokenStream {
    init(item)
}

#[cfg(feature = "attr")]
#[proc_macro_attribute]
pub fn pin_init_attr(_attr: TokenStream, item: TokenStream) -> TokenStream {
    pin_init(item)
}

#[proc_macro]
pub fn init(ts: TokenStream) -> TokenStream {
    inner::<false>(ts)
}

#[proc_macro]
pub fn pin_init(ts: TokenStream) -> TokenStream {
    inner::<true>(ts)
}

fn inner<const PIN: bool>(item: TokenStream) -> TokenStream {
    let mut tokens = item.into_iter().collect::<VecDeque<_>>();
    // first find the type and body that is being constructed
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

    // now lets extract each field

    // this is the actual initializer part
    let mut inner = vec![];
    // struct initializer to verify that every field has been initalized
    let mut check = vec![];
    // list of forget calls to forget the dropguards
    let mut forget = vec![];
    // are we creating a pinned initalizer or not?
    let initializer = if PIN {
        quote!(PinInitializer)
    } else {
        quote!(Initializer)
    };
    while let Some(Field { ident, expr }) = parse_field_init(&mut body) {
        let ident = TokenTree::Ident(ident);
        convert!(expr);
        inner.extend(quote! {
            // evaluate the expression
            let $ident = $expr;
            // call the initializer
            // SAFETY: place is valid, because we are inside of an initializer closure, we return
            //         when an error/panic occurs.
            unsafe { ::simple_safe_init::$initializer::init($ident, ::core::ptr::addr_of_mut!((*place).$ident))? };
            // create the drop guard
            // SAFETY: we forget the guard later when initialization has succeeded.
            let $ident = unsafe { ::simple_safe_init::DropGuard::new(::core::ptr::addr_of_mut!((*place).$ident)) };
        });
        check.extend(quote! {
            $ident: ::core::todo!(),
        });
        forget.extend(quote! {
            ::core::mem::forget($ident);
        });
    }
    convert!(ty, check, forget, inner);
    let init = if PIN { quote!(PinInit) } else { quote!(Init) };
    quote! {{
        let init = move |place: *mut $ty| {
            $inner
            #[allow(unreachable_code, clippy::diverging_sub_expression)]
            if false {
                let _: $ty = $ty {
                    $check
                };
            }
            $forget
            Ok(())
        };
        // SAFETY: either all fields have been initialized, or a compile error exists above
        unsafe { ::simple_safe_init::$init::from_closure(init) }
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
