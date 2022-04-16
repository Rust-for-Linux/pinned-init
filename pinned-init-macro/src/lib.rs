//! Proc macros for the [`pinned_init` crate], see  [`pinned_init`] and [`manual_init`]
//! for details.

use proc_macro2::*;
use proc_macro_error::*;
use quote::*;
use std::collections::*;
use syn::*;

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
/// - implements [`PinnedInit`] for your struct delegating to all fields marked
/// with `#[init]`.
/// - implements [`TransmuteInto<{your-struct-name}>`]
/// `for`{your-struct-name}Uninit` and checks for layout equivalence between the
/// two.
/// - creates a custom type borrowing from your struct that is used as the
/// `OngoingInit` type for the [`BeginInit`] trait.
/// - implements [`BeginInit`] for your struct.
///
/// Then you can safely, soundly and ergonomically initialize a value of such a
/// struct behind an [`OwnedUniquePtr<{your-struct-name}>`]:
/// TODO example
#[proc_macro_error]
#[proc_macro_attribute]
pub fn pinned_init(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let res = pinned_init_inner(parse_macro_input!(item as ItemStruct));
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
/// - implements [`TransmuteInto<{your-struct-name}>`]
/// `for`{your-struct-name}Uninit` and checks for layout equivalence between the
/// two.
/// - creates a custom type borrowing from your struct that is used as the
/// `OngoingInit` type for the [`BeginInit`] trait.
/// - implements [`BeginInit`] for your struct.
///
/// The only thing you need to implement is [`PinnedInit`].
///
/// Then you can safely, soundly and ergonomically initialize a value of such a
/// struct behind an [`OwnedUniquePtr<{your-struct-name}>`]:
/// TODO example
#[proc_macro_attribute]
#[proc_macro_error]
pub fn manual_init(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let res = manual_init_inner(input);
    res.into()
}

fn pinned_init_inner(
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
    let (impl_generics, type_generics, where_clause) = my_split_for_impl(&generics);
    let comma = if impl_generics.is_empty() {
        quote! {}
    } else {
        quote! {,}
    };
    let init_fields = fields
        .iter_mut()
        .filter(|f| {
            f.attrs
                .iter()
                .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init"))
        })
        .map(|f| {
            f.attrs.push(parse_quote! { #[pin] });
            f.ident.as_ref().unwrap().clone()
        })
        .collect::<Vec<_>>();
    quote! {
        // delegate to manual_init
        #[::pinned_init::manual_init]
        #(#attrs)*
        #vis #struct_token #ident #generics #fields #semi_token

        impl<#impl_generics> ::pinned_init::PinnedInit for #ident<#type_generics #comma false>
            #where_clause
        {
            type Initialized = #ident<#type_generics>;

            fn init_raw(this: ::pinned_init::needs_init::NeedsPinnedInit<Self>) {
                // just begin our init process and call init_raw on each field
                // marked with #[init]
                let this = ::pinned_init::needs_init::NeedsPinnedInit::begin_init(this);
                #(
                    ::pinned_init::PinnedInit::init_raw(this.#init_fields);
                )*
            }
        }
    }
}

fn manual_init_inner(
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
    // we need two generics for the two structs we are defining, both will have
    // an additional const bool parameter indicating init status.
    // The struct used to facilitate ergonomic and safe initialization needs a
    // lifetime that is named `'__ongoing_init`.
    let init_ident = format_ident!("__INIT");
    let ongoing_init_lifetime = quote! { '__ongoing_init };
    let where_clause = generics.make_where_clause();
    if !where_clause.predicates.empty_or_trailing() {
        where_clause.predicates.push_punct(Default::default());
    }
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
    let ongoing_init_fields = make_ongoing_init_fields(fields.clone(), &ongoing_init_lifetime);
    let type_params = generics.type_params().map(|p| &p.ident).collect::<Vec<_>>();
    // go through all of the fields in this struct and for each where `#[init]`
    // is specified
    // - append the `__INIT` expression to the generics of that type
    let bare_fields = fields
        .iter()
        .filter(|f| {
            !f.attrs
                .iter()
                .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("pin"))
                && !f
                    .attrs
                    .iter()
                    .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init"))
        })
        .map(|f| f.ident.as_ref().unwrap().clone())
        .collect::<Vec<_>>();
    let pinned_fields = fields
        .iter_mut()
        .filter(|f| {
            f.attrs
                .iter()
                .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("pin"))
                && !f
                    .attrs
                    .iter()
                    .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init"))
        })
        .map(|f| f.ident.as_ref().unwrap().clone())
        .collect::<Vec<_>>();
    let pinned_init_fields = fields
        .iter_mut()
        .filter(|f| {
            f.attrs
                .iter()
                .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("pin"))
                && f.attrs
                    .iter()
                    .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init"))
        })
        .map(|f| {
            f.attrs
                .retain(|a| !(matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init")));
            append_generics(&mut f.ty, &init_ident);
            f.ident.as_ref().unwrap().clone()
        })
        .collect::<Vec<_>>();
    let bare_init_fields = fields
        .iter_mut()
        .filter(|f| {
            !f.attrs
                .iter()
                .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("pin"))
                && f.attrs
                    .iter()
                    .any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init"))
        })
        .map(|f| {
            f.attrs
                .retain(|a| !(matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init")));
            append_generics(&mut f.ty, &init_ident);
            f.ident.as_ref().unwrap().clone()
        })
        .collect::<Vec<_>>();
    let all_fields = bare_fields
        .iter()
        .chain(pinned_fields.iter())
        .chain(bare_init_fields.iter())
        .chain(pinned_init_fields.iter())
        .cloned()
        .collect::<Vec<_>>();
    let field_offset_check_name = all_fields
        .iter()
        .map(|i| {
            Ident::new(
                &format!("__check_valid_offset_between_uninit_and_init_{}", i).to_uppercase(),
                i.span(),
            )
        })
        .collect::<Vec<_>>();
    quote! {
        // pin_project the original struct
        #[::pinned_init::__private::pin_project]
        #(#attrs)*
        // add const parameter with default value true
        #vis #struct_token #ident <#impl_generics #comma const #init_ident: bool = true> #where_clause #fields

        // define the type alias
        #vis type #uninit_ident <#impl_generics> = #ident<#type_generics #comma false>;

        // define a new struct used to handle the ongoing initialization.
        #vis #struct_token #ongoing_init_ident <#ongoing_init_lifetime #comma #impl_generics>
        #where_clause
            #(#type_params: #ongoing_init_lifetime,)*
        #ongoing_init_fields


        // define constants to ensure the layout between init and uninit is the
        // same
        #[allow(non_uppercase_globals)]
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
            #(
                const #field_offset_check_name: () = unsafe {
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
        }

        impl <#impl_generics> ::pinned_init::private::BeginInit for #uninit_ident<#type_generics>
        #where_clause
        {
            type OngoingInit<#ongoing_init_lifetime> = #ongoing_init_ident <#ongoing_init_lifetime #comma #type_generics>
            where
                #(#type_params: #ongoing_init_lifetime,)*
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
                #(Self::#field_offset_check_name;)*
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

        // implement TransmuteInto because we checked the layout before.
        unsafe impl<#impl_generics> ::pinned_init::transmute::TransmuteInto<#ident<#type_generics>> for
        #uninit_ident<#type_generics>
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
    }
}

/// splits the given generics into impl_generics, type_generics and the where
/// clause, similar to [`Generics::split_for_impl`], but does not produce the '<' and '>' tokens,
/// in order to allow generic extensions
fn my_split_for_impl(generics: &Generics) -> (TokenStream, TokenStream, Option<&WhereClause>) {
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

/// Changes the types of the given fields for use in the [`BeginInit::OngoingInit`] type.
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
                    let mut ty = f.ty;
                    if f.attrs.iter().any(|a| matches!(a.style, AttrStyle::Outer) && a.path.is_ident("init")) {
                        append_generics::<Expr>(&mut ty, &parse_quote! { false });
                        if f.attrs.iter().any(|a|{
                            matches!(a.style, AttrStyle::Outer)
                                && a.path.is_ident("pin")
                        }) {
                            f.ty = parse_quote! { ::pinned_init::needs_init::NeedsPinnedInit<#ongoing_init_lifetime, #ty> };
                        } else {
                            f.ty = parse_quote! { ::pinned_init::needs_init::NeedsInit<#ongoing_init_lifetime, #ty> };
                        }
                    } else {
                        if f.attrs.iter().any(|a|{
                            matches!(a.style, AttrStyle::Outer)
                                && a.path.is_ident("pin")
                        }) {
                            f.ty = parse_quote! { ::core::pin::Pin<&#ongoing_init_lifetime mut #ty> };
                        } else {
                            f.ty = parse_quote! { &#ongoing_init_lifetime mut #ty };
                        }
                    }
                    f.attrs.retain(|a|
                        !(matches!(a.style, AttrStyle::Outer)
                            && (a.path.is_ident("init") || a.path.is_ident("pin"))));
                    f
                })
                .collect();
            Fields::Named(FieldsNamed { named, brace_token })
        }
        _ => panic!("Expected named fields!"),
    }
}

/// Append the expression as a const generic parameter to the generics of the given
/// type.
fn append_generics<Expr: ToTokens>(ty: &mut Type, expr: &Expr) {
    match ty {
        Type::Path(TypePath {path, ..}) => {
            if let Some(PathSegment {arguments, ..}) = path.segments.last_mut() {
                match arguments {
                    PathArguments::None => {
                        *arguments = PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            colon2_token: None,
                            lt_token: <Token![<]>::default(),
                            args: parse_quote! { #expr },
                            gt_token: <Token![>]>::default()
                        });
                    }
                    PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        args, ..
                    }) => {
                        args.push(GenericArgument::Const(parse_quote! { #expr }));
                    }
                    PathArguments::Parenthesized(p) => {
                        emit_error!(p, "Expected arguments with angled brackets.");
                    }
                }
            } else {
                emit_error!(path, "Expected at least one ident.");
            }
        }
        rest => emit_error!(rest, "Cannot #[init] this type, expected a type path."),
    }
}
