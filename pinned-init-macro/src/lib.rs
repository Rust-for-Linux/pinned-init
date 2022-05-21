//! Proc macros for the `pinned_init` crate, see  [`macro@pinned_init`] and [`macro@manual_init`]
//! for details.

use crate::helpers::{has_outer_attr, my_split_for_impl, parse_attrs, ManualInitParam};
use proc_macro2::*;
use proc_macro_error::*;
use quote::*;
use syn::{parse::*, *};

mod helpers;

/// Use this attribute on a struct with named fields to ensure safe
/// pinned initialization of all the fields marked with `#[init]`.
///
/// This attribute does several things, it:
/// - `#[pin_project]`s your struct, structually pinning all fields with `#[init]` implicitly (adding `#[pin]`).
/// - adds a constant type parameter of type bool with a default value of true.
/// This constant type parameter indicates if your struct is in an initialized
/// (and thus also pinned) state. A type alias `{your-struct-name}Uninit` is
/// created to refer to the uninitialized variant more ergonomically, it should
/// always be used instead of specifying the const parameter.
/// - propagates that const parameter to all fields marked with `#[init]`.
/// - implements `PinnedInit` for your struct delegating to all fields marked
/// with `#[init]`.
/// - implements `TransmuteInto<{your-struct-name}>`()
/// `for`{your-struct-name}Uninit` and checks for layout equivalence between the
/// two.
/// - creates a custom type borrowing from your struct that is used as the
/// `OngoingInit` type for the `BeginPinnedInit` trait.
/// - implements `BeginPinnedInit` for your struct.
///
/// Then you can safely, soundly and ergonomically initialize a value of such a
/// struct behind an `OwnedUniquePtr<{your-struct-name}>`:
/// TODO example
#[proc_macro_error]
#[proc_macro_attribute]
pub fn pinned_init(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let res = pinned_init_inner(attr.into(), parse_macro_input!(item as ItemStruct));
    res.into()
}

/// Use this attribute on a struct with named fields to ensure safer
/// pinned initialization of all the fields marked with `#[init]`.
///
/// This attribute does several things, it:
/// - `#[pin_project]`s your struct, structually pinning all fields with `#[pin]`.
/// - adds a constant type parameter of type bool with a default value of true.
/// This constant type parameter indicates if your struct is in an initialized
/// (and thus also pinned) state. A type alias `{your-struct-name}Uninit` is
/// created to refer to the uninitialized variant more ergonomically, it should
/// always be used instead of specifying the const parameter.
/// - propagates that const parameter to all fields marked with `#[init]`.
/// - implements `TransmuteInto<{your-struct-name}>`
/// `for`{your-struct-name}Uninit` and checks for layout equivalence between the
/// two.
/// - creates a custom type borrowing from your struct that is used as the
/// `OngoingInit` type for the `BeginPinnedInit` trait.
/// - implements `BeginPinnedInit` for your struct.
///
/// The only thing you need to implement is `PinnedInit`.
///
/// Then you can safely, soundly and ergonomically initialize a value of such a
/// struct behind an `OwnedUniquePtr<{your-struct-name}>`:
/// TODO example
#[proc_macro_attribute]
#[proc_macro_error]
pub fn manual_init(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let res = manual_init_inner(attr.into(), input);
    res.into()
}

#[proc_macro_derive(BeginInit, attributes(ongoing_init, init))]
#[proc_macro_error]
pub fn derive_begin_init(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    derive_begin_init_inner(input).into()
}

#[proc_macro_derive(BeginPinnedInit, attributes(ongoing_init, init, pin))]
#[proc_macro_error]
pub fn derive_begin_pinned_init(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    derive_begin_pinned_init_inner(input).into()
}

fn pinned_init_inner(
    attr: TokenStream,
    ItemStruct {
        attrs,
        vis,
        struct_token,
        ident,
        generics,
        mut fields,
        semi_token,
    }: ItemStruct,
) -> TokenStream {
    // Only structs with named fields are supported.
    // To provide a better debugging experience, we only emit an error and
    // correct the fields value.
    if !matches!(fields, Fields::Named(_)) {
        emit_error!(fields, "Expected named fields with '{{}}'");
        fields = match fields {
            Fields::Unit => Fields::Named(parse_quote! { {} }),
            Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => Fields::Named(FieldsNamed {
                named: unnamed
                    .into_iter()
                    .enumerate()
                    .map(|(i, mut f)| {
                        f.ident = Some(format_ident!("__field{}", i));
                        f
                    })
                    .collect(),
                brace_token: Default::default(),
            }),
            Fields::Named(_) => unreachable!(),
        };
    }
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let (init_fields, (pinned_field_types, param_pos)): (Vec<_>, (Vec<_>, Vec<Member>)) = fields
        .iter_mut()
        .filter(|f| {
            f.attrs
                .iter()
                .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init"))
        })
        .enumerate()
        .map(|(i, f)| {
            f.attrs.push(parse_quote! { #[pin] });
            let ty = f.ty.clone();
            (
                f.ident.as_ref().unwrap().clone(),
                (
                    ty,
                    Member::Unnamed(Index {
                        index: i.try_into().unwrap(),
                        span: Span::call_site(),
                    }),
                ),
            )
        })
        .unzip();
    let attr_comma = if attr.is_empty() {
        quote! {}
    } else {
        quote! {,}
    };
    let uninit_ident = format_ident!("{}Uninit", ident);
    quote! {
        // delegate to manual_init
        #[::pinned_init::manual_init(pinned #attr_comma #attr)]
        #(#attrs)*
        #vis #struct_token #ident #generics #fields #semi_token

        impl #impl_generics ::pinned_init::PinnedInit for #uninit_ident #type_generics
            #where_clause
        {
            type Initialized = #ident #type_generics;
            // TODO correct param stuff
            type Param = (#(<<#pinned_field_types as ::pinned_init::private::AsUninit>::Uninit as ::pinned_init::PinnedInit>::Param),*,);

            fn init_raw(this: ::pinned_init::needs_init::NeedsPinnedInit<Self>, param: Self::Param) {
                // just begin our init process and call init_raw on each field
                // marked with #[init]
                let this = ::pinned_init::needs_init::NeedsPinnedInit::begin_init(this);
                #(
                    ::pinned_init::PinnedInit::init_raw(this.#init_fields, param.#param_pos);
                )*
            }
        }
    }
}

fn manual_init_inner(
    attr: TokenStream,
    ItemStruct {
        attrs,
        vis,
        struct_token,
        ident,
        mut generics,
        mut fields,
        semi_token: _,
    }: ItemStruct,
) -> TokenStream {
    let my_attrs = match parse_attrs.parse2(attr) {
        Ok(attrs) => attrs,
        Err(e) => return e.to_compile_error(),
    };
    let is_pinned = my_attrs
        .iter()
        .any(|p| matches!(p, ManualInitParam::Pinned));
    let pin_project_attrs = my_attrs
        .into_iter()
        .filter_map(|p| {
            if let ManualInitParam::PinProject(raw) = p {
                Some(raw)
            } else {
                None
            }
        })
        .reduce(|a, b| quote! { #a #b })
        .map(|a| quote! { (#a) });
    if !is_pinned && pin_project_attrs.is_some() {
        emit_error!(
            pin_project_attrs.as_ref().unwrap(),
            "Pinned attribute not supplied, pin_project is not applied."
        );
    }
    // Only structs with named fields are supported.
    // To provide a better debugging experience, we only emit an error and
    // correct the fields value.
    if !matches!(fields, Fields::Named(_)) {
        emit_error!(fields, "Expected named fields with '{{}}'");
        fields = match fields {
            Fields::Unit => Fields::Named(parse_quote! { {} }),
            Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => Fields::Named(FieldsNamed {
                named: unnamed
                    .into_iter()
                    .enumerate()
                    .map(|(i, mut f)| {
                        f.ident = Some(format_ident!("__field{}", i));
                        f
                    })
                    .collect(),
                brace_token: Default::default(),
            }),
            Fields::Named(_) => unreachable!(),
        };
    }
    let uninit_ident = format_ident!("{}Uninit", ident);
    let ongoing_init_ident = format_ident!("{}OngoingInit", ident);
    // `'__ongoing_init` for the OngoingInit type
    let ongoing_init_lifetime = quote! { '__ongoing_init };
    // add a where clause that is empty or trailing
    let where_clause = generics.make_where_clause();
    if !where_clause.predicates.empty_or_trailing() {
        where_clause.predicates.push_punct(Default::default());
    }
    // ensure that we have a `where`
    let where_clause = if where_clause.predicates.is_empty() {
        quote! {where}
    } else {
        quote! { #where_clause }
    };
    let (impl_generics, type_generics, _) = my_split_for_impl(&generics);
    let comma = if impl_generics.is_empty() {
        quote! {}
    } else {
        quote! {,}
    };
    let uninit_fields = make_uninit_fields(fields.clone());
    let ongoing_init_fields =
        make_ongoing_init_fields(uninit_fields.clone(), &ongoing_init_lifetime);
    let all_fields = fields
        .iter_mut()
        .map(|f| {
            f.attrs.retain(|a| {
                !(matches!(a.style, AttrStyle::Outer)
                    && (a.path.is_ident("init") || a.path.is_ident("uninit")))
            });
            f.ident.as_ref().unwrap().clone()
        })
        .collect::<Vec<_>>();
    let check_mod = quote! {
        // define constants to ensure the layout between init and uninit is the
        // same
        impl <#impl_generics> #uninit_ident<#type_generics> {
            const __CHECK_ALIGNMENT: () = {
                if ::core::mem::align_of::<#uninit_ident<#type_generics>>() != ::core::mem::align_of::<#ident<#type_generics>>() {
                    panic!(concat!("The alignments of the uninitialized and initialized variants of the type `", stringify!(#ident<#type_generics>), "` are not identical."));
                }
            };
            const __CHECK_SIZE: () = {
                if ::core::mem::size_of::<#uninit_ident<#type_generics>>() != ::core::mem::size_of::<#ident<#type_generics>>() {
                    panic!(concat!("The sizes of the uninitialized and initialized variants of the type `", stringify!(#ident<#type_generics>), "` are not identical."));
                }
            };
            const __CHECK_OFFSETS: () = {
                #(
                    unsafe {
                        // create a valid allocation of uninit, we cannot use null
                        // here, because that would be undefined behaviour
                        let uninit = ::core::mem::MaybeUninit::<#uninit_ident<#type_generics>>::uninit();
                        let u = uninit.as_ptr();
                        // reinterpret the pointer
                        let i = u as *const #ident<#type_generics>;
                        // get each offset using the `offset_from` function, because
                        // this function takes a *const T pointer, we cast both to
                        // *const u8
                        let u_off = (::core::ptr::addr_of!((*u).#all_fields) as *const u8).offset_from(u as *const u8);
                        let i_off = (::core::ptr::addr_of!((*i).#all_fields) as *const u8).offset_from(i as *const u8);
                        if u_off != i_off {
                            panic!(concat!("The offset of `", stringify!(#all_fields), "` is not the same between the uninitialized and initialized variants of the type `", stringify!(#ident<#type_generics>), "`."));
                        }
                    };
                )*
            };
        }
    };
    let pin_project = if is_pinned {
        quote! { #[::pinned_init::__private::pin_project #pin_project_attrs] }
    } else {
        quote! {}
    };
    let begin_init = if is_pinned {
        quote! {#[derive(::pinned_init::private::BeginPinnedInit)]}
    } else {
        quote! {#[derive(::pinned_init::private::BeginInit)]}
    };
    if is_pinned {
    } else {
    }
    quote! {
        #[repr(C)]
        #pin_project
        #(#attrs)*
        #vis #struct_token #ident <#impl_generics> #where_clause #fields

        #[repr(C)]
        #begin_init
        #pin_project
        #[ongoing_init(#ongoing_init_ident)]
        #(#attrs)*
        #vis #struct_token #uninit_ident <#impl_generics> #where_clause #uninit_fields

        #check_mod

        // define a new struct used to handle the ongoing initialization.
        // allow dead_code, because some fields may not be used in initialization.
        #[allow(dead_code)]
        #vis #struct_token #ongoing_init_ident <#ongoing_init_lifetime #comma #impl_generics>
        #where_clause
            Self: #ongoing_init_lifetime,
        #ongoing_init_fields

        // implement TransmuteInto because we implement #[repr(C)] and all field types are either the same,
        // or TransmuteInto with their uninit variants.
        unsafe impl<#impl_generics> ::pinned_init::transmute::TransmuteInto<#ident<#type_generics>> for #uninit_ident<#type_generics>
        #where_clause
        {
            unsafe fn transmute_ptr(this: *const Self) ->
                *const #ident<#type_generics>
            {
                unsafe {
                    ::core::mem::transmute(this)
                }
            }
        }

        unsafe impl<#impl_generics> ::pinned_init::private::AsUninit for #ident<#type_generics> #where_clause {
            type Uninit = #uninit_ident<#type_generics>;
        }
    }
}

fn derive_begin_init_inner(
    DeriveInput {
        attrs,
        vis: _,
        ident,
        generics,
        data,
    }: DeriveInput,
) -> TokenStream {
    let fields;
    if let Data::Struct(s) = data {
        fields = s.fields;
    } else {
        abort!(ident, "Can only derive BeginInit for structs.");
    }
    let (impl_generics, type_generics, where_clause) = my_split_for_impl(&generics);
    let comma = if type_generics.is_empty() {
        quote! {}
    } else {
        quote! {,}
    };
    let ongoing_init_lifetime = quote! {'__ongoing_init};
    let ongoing_init_ident = attrs
        .iter()
        .filter_map(|a| {
            if let Ok(Meta::List(MetaList { path, nested, .. })) = a.parse_meta() {
                if path.is_ident("ongoing_init") {
                    if nested.len() == 1 {
                        if let NestedMeta::Meta(Meta::Path(path)) = nested.first().unwrap() {
                            return Some(path.clone());
                        } else {
                            emit_error!(nested, "Expected a path.");
                        }
                    } else {
                        emit_error!(nested, "Expected single argument");
                    }
                }
            }
            None
        })
        .reduce(|a, b| {
            emit_error!(b, "#[ongoing_init] should only be specified once."; note = SpanRange::from_tokens(&a).collapse() => "other #[ongoing_init] here");
            a
        }).unwrap_or_else(|| abort!(ident, "Expected #[ongoing_init(<name>)] attribute."));
    let mut bare_fields = vec![];
    let mut bare_init_fields = vec![];

    for field in fields.iter() {
        if has_outer_attr(field.attrs.iter(), "init") {
            bare_init_fields.push(field.ident.as_ref().unwrap().clone());
        } else {
            bare_fields.push(field.ident.as_ref().unwrap().clone());
        }
    }
    quote! {
        impl <#impl_generics> ::pinned_init::private::BeginInit for #ident<#type_generics>
        #where_clause
        {
            type OngoingInit<#ongoing_init_lifetime> = #ongoing_init_ident <#ongoing_init_lifetime #comma #type_generics>
            where
                Self: #ongoing_init_lifetime,
            ;

            #[inline]
            unsafe fn __begin_init<#ongoing_init_lifetime>(self: &#ongoing_init_lifetime mut Self) -> Self::OngoingInit<#ongoing_init_lifetime>
            where
                Self: #ongoing_init_lifetime,
            {
                // need to mention these constants again, because they are not
                // computed if they are not used. If no one uses the
                // __begin_init function, then they will not use this library's
                // TransmuteInto functionality and so they will be on their own.
                Self::__CHECK_ALIGNMENT;
                Self::__CHECK_SIZE;
                Self::__CHECK_OFFSETS;
                unsafe {
                    #ongoing_init_ident {
                        #(#bare_fields: &mut self.#bare_fields,)*
                        #(#bare_init_fields: ::pinned_init::needs_init::NeedsInit::new_unchecked(&mut self.#bare_init_fields),)*
                    }
                }
            }
        }
    }
}

fn derive_begin_pinned_init_inner(
    DeriveInput {
        attrs,
        vis: _,
        ident,
        generics,
        data,
    }: DeriveInput,
) -> TokenStream {
    let fields;
    if let Data::Struct(s) = data {
        fields = s.fields;
    } else {
        abort!(ident, "Can only derive BeginPinnedInit for structs.");
    }
    let (impl_generics, type_generics, where_clause) = my_split_for_impl(&generics);
    let comma = if type_generics.is_empty() {
        quote! {}
    } else {
        quote! {,}
    };
    let ongoing_init_lifetime = quote! {'__ongoing_init};
    let ongoing_init_ident = attrs
        .iter()
        .filter_map(|a| {
            if let Ok(Meta::List(MetaList { path, nested, .. })) = a.parse_meta() {
                if path.is_ident("ongoing_init") {
                    if nested.len() == 1 {
                        if let NestedMeta::Meta(Meta::Path(path)) = nested.first().unwrap() {
                            return Some(path.clone());
                        } else {
                            emit_error!(nested, "Expected a path.");
                        }
                    } else {
                        emit_error!(nested, "Expected single argument");
                    }
                }
            }
            None
        })
        .reduce(|a, b| {
            emit_error!(b, "#[ongoing_init] should only be specified once."; note = SpanRange::from_tokens(&a).collapse() => "other #[ongoing_init] here");
            a
        }).unwrap_or_else(|| abort!(ident, "Expected #[ongoing_init(<name>)] attribute."));
    let mut bare_fields = vec![];
    let mut pinned_fields = vec![];
    let mut bare_init_fields = vec![];
    let mut pinned_init_fields = vec![];

    for field in fields.iter() {
        match (
            has_outer_attr(field.attrs.iter(), "init"),
            has_outer_attr(field.attrs.iter(), "pin"),
        ) {
            (true, true) => pinned_init_fields.push(field.ident.as_ref().unwrap().clone()),
            (false, true) => pinned_fields.push(field.ident.as_ref().unwrap().clone()),
            (true, false) => bare_init_fields.push(field.ident.as_ref().unwrap().clone()),
            (false, false) => bare_fields.push(field.ident.as_ref().unwrap().clone()),
        }
    }
    quote! {
        impl <#impl_generics> ::pinned_init::private::BeginPinnedInit for #ident<#type_generics>
        #where_clause
        {
            type OngoingInit<#ongoing_init_lifetime> = #ongoing_init_ident <#ongoing_init_lifetime #comma #type_generics>
            where
                Self: #ongoing_init_lifetime,
            ;

            #[inline]
            unsafe fn __begin_init<#ongoing_init_lifetime>(self: ::core::pin::Pin<&#ongoing_init_lifetime mut Self>) -> Self::OngoingInit<#ongoing_init_lifetime>
            where
                Self: #ongoing_init_lifetime,
            {
                // need to mention these constants again, because they are not
                // computed if they are not used. If no one uses the
                // __begin_init function, then they will not use this library's
                // TransmuteInto functionality and so they will be on their own.
                Self::__CHECK_ALIGNMENT;
                Self::__CHECK_SIZE;
                Self::__CHECK_OFFSETS;
                let this = self.project();
                unsafe {
                    #ongoing_init_ident {
                        #(#bare_fields: this.#bare_fields,)*
                        #(#pinned_fields: this.#pinned_fields,)*
                        #(#bare_init_fields: ::pinned_init::needs_init::NeedsInit::new_unchecked(this.#bare_init_fields),)*
                        #(#pinned_init_fields: ::pinned_init::needs_init::NeedsPinnedInit::new_unchecked(this.#pinned_init_fields),)*
                    }
                }
            }
        }
    }
}

/// Changes the types of the given fields for use in the [`BeginPinnedInit::OngoingInit`] type.
/// it handles four cases:
/// - `#[init] #[pin] => <T as AsUninit>::Uninit`
/// - `#[init] => <T as AsUninit>::Uninit`
/// - `else => T`
fn make_uninit_fields(fields: Fields) -> Fields {
    match fields {
        Fields::Named(FieldsNamed {
            mut named,
            brace_token,
        }) => {
            named = named
                .into_iter()
                .map(|mut f| {
                    if has_outer_attr(f.attrs.iter(), "init") {
                        if let Some(a@Attribute { tokens, .. } ) = f.attrs.iter()
                            .filter(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("uninit"))
                            .reduce(|a, b| {
                                emit_error!(a, "Expected at most one #[uninit = <type>] attribute."; note = SpanRange::from_tokens(&b).collapse() => "Other found here.");
                                a
                            })
                        {
                            let mut tokens = tokens.clone().into_iter();
                            if let Some(TokenTree::Punct(p)) = tokens.next()  {
                                if p.as_char() == '=' {
                                    // consume the '='
                                } else {
                                    emit_error!(a, "Expected #[uninit = <type>].");
                                    tokens = quote!{ () }.into_iter();
                                }
                            } else {
                                emit_error!(a, "Expected #[uninit = <type>].");
                                tokens = quote!{ () }.into_iter();
                            }
                            let tokens = TokenStream::from_iter(tokens);
                            f.ty = parse_quote! { #tokens };
                        } else {
                            let ty = f.ty;
                            f.ty = parse_quote! { <#ty as ::pinned_init::private::AsUninit>::Uninit };
                        }
                    }
                    f.attrs.retain(|a| !(matches!(a.style, AttrStyle::Outer) && a.path.is_ident("uninit")));
                    f
                })
                .collect();
            Fields::Named(FieldsNamed { named, brace_token })
        }
        _ => panic!("Expected named fields!"),
    }
}

/// Changes the types of the given fields for use in the [`BeginPinnedInit::OngoingInit`] type.
/// it handles four cases:
/// - `#[init] #[pin] => NeedsPinnedInit<T>`
/// - `#[pin] => Pin<&mut T>`
/// - `#[init] => NeedsInit<T>`
/// - `none => &mut T`
fn make_ongoing_init_fields(fields: Fields, ongoing_init_lifetime: &TokenStream) -> Fields {
    match fields {
        Fields::Named(FieldsNamed {
            mut named,
            brace_token,
        }) => {
            named = named
                .into_iter()
                .map(|mut f| {
                    let ty = f.ty;
                    f.ty = match (has_outer_attr(f.attrs.iter(), "init"), has_outer_attr(f.attrs.iter(), "pin")) {
                        (true, true) => parse_quote! { ::pinned_init::needs_init::NeedsPinnedInit<#ongoing_init_lifetime, #ty> },
                        (true, false) => parse_quote! { ::pinned_init::needs_init::NeedsInit<#ongoing_init_lifetime, #ty> },
                        (false, true) => parse_quote! { ::core::pin::Pin<&#ongoing_init_lifetime mut #ty> },
                        (false, false) => parse_quote! { &#ongoing_init_lifetime mut #ty },
                    };
                    f.attrs.retain(|a| !(matches!(a.style, AttrStyle::Outer) && (a.path.is_ident("init") || a.path.is_ident("pin"))));
                    f
                })
                .collect();
            Fields::Named(FieldsNamed { named, brace_token })
        }
        _ => panic!("Expected named fields!"),
    }
}
