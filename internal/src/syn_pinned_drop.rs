// SPDX-License-Identifier: Apache-2.0 OR MIT

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, Error, ImplItem, ImplItemFn, ItemImpl,
    Result, Token,
};

pub(crate) fn pinned_drop(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    parse_macro_input!(args as syn::parse::Nothing);
    do_impl(parse_macro_input!(input as ItemImpl))
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

fn do_impl(mut input: ItemImpl) -> Result<TokenStream> {
    let Some((_, path, _)) = &mut input.trait_ else {
        return Err(Error::new_spanned(
            input,
            "expected an `impl` block implementing `PinnedDrop`",
        ));
    };
    if !is_pinned_drop(path) {
        return Err(Error::new_spanned(
            input,
            "expected an `impl` block implementing `PinnedDrop`",
        ));
    }
    let mut error = None;
    if let Some(unsafety) = input.unsafety.take() {
        error = Some(
            Error::new_spanned(
                unsafety,
                "implementing the trait `PinnedDrop` via `#[pinned_drop]` is not unsafe",
            )
            .into_compile_error(),
        );
    }
    input.unsafety = Some(Token![unsafe](input.impl_token.span()));
    if path.segments.len() != 2 {
        path.segments.insert(0, parse_quote!(pin_init));
    }
    path.leading_colon.get_or_insert(Token![::](path.span()));
    for item in &mut input.items {
        match item {
            ImplItem::Fn(ImplItemFn { sig, .. }) if sig.ident == "drop" => {
                sig.inputs
                    .push(parse_quote!(_: ::pin_init::__internal::OnlyCallFromDrop));
            }
            _ => {}
        }
    }
    Ok(quote! {
        #error
        #input
    })
}

fn is_pinned_drop(path: &syn::Path) -> bool {
    if path.segments.len() > 2 {
        return false;
    }
    // If there is a `::`, then the path needs to be `::pin_init::PinnedDrop`.
    if path.leading_colon.is_some() && path.segments.len() != 2 {
        return false;
    }
    for (actual, expected) in path.segments.iter().rev().zip(["PinnedDrop", "pin_init"]) {
        if actual.ident != expected {
            return false;
        }
    }
    true
}
