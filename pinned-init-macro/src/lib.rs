// Documentation of these macros is in `pinned-init`.

mod init;
mod pin_data;
mod pinned_drop;
mod zeroable;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_attribute]
pub fn pin_data(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    pin_data::pin_data(args.into(), input)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn pinned_drop(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    parse_macro_input!(args as syn::parse::Nothing);
    pinned_drop::pinned_drop(input)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

#[proc_macro_derive(Zeroable)]
pub fn derive_zeroable(input: TokenStream) -> TokenStream {
    let raw_input = input.clone();
    let input = parse_macro_input!(input as DeriveInput);
    zeroable::derive(input, raw_input.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

#[proc_macro]
pub fn init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    init::init(input, true, false)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

#[proc_macro]
pub fn try_init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    init::init(input, false, false)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

#[proc_macro]
pub fn pin_init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    init::init(input, true, true)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

#[proc_macro]
pub fn try_pin_init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    init::init(input, false, true)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}
