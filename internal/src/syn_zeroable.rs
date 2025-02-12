// SPDX-License-Identifier: Apache-2.0 OR MIT

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, Data, DataStruct, DeriveInput, Error, GenericParam, Result,
    TypeParam, WherePredicate,
};

pub(crate) fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let raw = input.clone().into();
    do_derive(parse_macro_input!(input as DeriveInput), raw)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

fn do_derive(
    DeriveInput {
        ident,
        mut generics,
        data,
        ..
    }: DeriveInput,
    raw_input: TokenStream,
) -> Result<TokenStream> {
    let Data::Struct(DataStruct { fields, .. }) = data else {
        return Err(Error::new_spanned(
            raw_input,
            "`Zeroable` can only be derived for structs.",
        ));
    };
    let field_ty = fields.iter().map(|f| &f.ty);
    let zeroable_bounds = generics
        .params
        .iter()
        .filter_map(|p| match p {
            GenericParam::Type(TypeParam { ident, .. }) => {
                Some(parse_quote!(#ident: ::pin_init::Zeroable))
            }
            _ => None,
        })
        .collect::<Vec<WherePredicate>>();
    generics
        .make_where_clause()
        .predicates
        .extend(zeroable_bounds);
    let (impl_generics, ty_generics, whr) = generics.split_for_impl();
    Ok(quote! {
        // SAFETY: Every field type implements `Zeroable` and padding bytes may be zero.
        #[automatically_derived]
        unsafe impl #impl_generics ::pin_init::Zeroable for #ident #ty_generics
            #whr
        {}
        const _: () = {
            fn assert_zeroable<T: ?::core::marker::Sized + ::pin_init::Zeroable>() {}
            fn ensure_zeroable #impl_generics ()
                #whr
            {
                #(assert_zeroable::<#field_ty>();)*
            }
        };
    })
}
