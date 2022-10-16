mod pin_project;
mod pinned_drop;

use proc_macro::TokenStream;

/// Used to specify the pin information of the fields of a struct.
///
/// This is somewhat similar in purpose as
/// [pin-project-lite](https://crates.io/crates/pin-project-lite).
/// Place this macro on a struct definition and then `#[pin]` in front of the attributes of each
/// field you want to have structurally pinned.
///
/// # Examples
///
/// ```rust,ignore
/// #[pin_project]
/// struct A {
///     #[pin]
///     a: usize,
/// }
/// ```
#[proc_macro_attribute]
pub fn pin_project(args: TokenStream, item: TokenStream) -> TokenStream {
    pin_project::pin_project(args, item)
}

/// TODO
#[proc_macro_attribute]
pub fn pinned_drop(args: TokenStream, input: TokenStream) -> TokenStream {
    pinned_drop::pinned_drop(args, input)
}
