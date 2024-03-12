use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, Data, DataStruct, DeriveInput, GenericParam, TypeParam,
    WherePredicate,
};

pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        mut generics,
        data,
        ..
    } = parse_macro_input!(input as DeriveInput);
    let zeroable_bounds = generics
        .params
        .iter()
        .filter_map(|p| match p {
            GenericParam::Type(t) => Some(t),
            _ => None,
        })
        .map(|TypeParam { ident, .. }| {
            parse_quote! { #ident: ::pinned_init::Zeroable, }
        })
        .collect::<Vec<WherePredicate>>();
    generics
        .make_where_clause()
        .predicates
        .extend(zeroable_bounds);
    let (impl_g, type_g, whr) = generics.split_for_impl();
    let Data::Struct(DataStruct { fields, .. }) = data else {
        panic!("expected struct")
    };
    let field_ty = fields.iter().map(|f| &f.ty);
    quote! {
        // SAFETY: Every field type implements `Zeroable` and padding bytes may be zero.
        #[automatically_derived]
        unsafe impl #impl_g ::pinned_init::Zeroable for #ident #type_g
            #whr
        {}
        const _: () = {
            fn assert_zeroable<T: ?::core::marker::Sized + ::pinned_init::Zeroable>() {}
            fn ensure_zeroable #impl_g ()
                #whr
            {
                #(assert_zeroable::<#field_ty>();)*
            }
        };
    }
    .into()
}
