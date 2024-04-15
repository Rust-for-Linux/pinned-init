mod init;
mod pin_data;
mod pinned_drop;
mod zeroable;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

/// Used to specify the pinning information of the fields of a struct.
///
/// This is somewhat similar in purpose as
/// [pin-project-lite](https://crates.io/crates/pin-project-lite).
/// Place this macro on a struct definition and then `#[pin]` in front of the attributes of each
/// field you want to have structurally pinned.
///
/// This macro enables the use of the [`pin_init!`] macro. When pinned-initializing a `struct`,
/// then `#[pin]` directs the type of intializer that is required.
///
/// If your `struct` implements `Drop`, then you need to add `PinnedDrop` as arguments to this
/// macro, and change your `Drop` implementation to `PinnedDrop` annotated with
/// `#[`[`macro@pinned_drop`]`]`, since dropping pinned values requires extra care.
///
/// # Examples
///
/// ```rust,ignore
/// #[pin_data]
/// struct DriverData {
///     #[pin]
///     queue: Mutex<Vec<Command>>,
///     buf: Box<[u8; 1024 * 1024]>,
/// }
/// ```
///
/// ```rust,ignore
/// #[pin_data(PinnedDrop)]
/// struct DriverData {
///     #[pin]
///     queue: Mutex<Vec<Command>>,
///     buf: Box<[u8; 1024 * 1024]>,
///     raw_info: *mut Info,
/// }
///
/// #[pinned_drop]
/// impl PinnedDrop for DriverData {
///     fn drop(self: Pin<&mut Self>) {
///         unsafe { bindings::destroy_info(self.raw_info) };
///     }
/// }
/// ```
///
/// [`pin_init!`]: ../pinned_init/macro.pin_init.html
//  ^ cannot use direct link, since `kernel` is not a dependency of `macros`
#[proc_macro_attribute]
pub fn pin_data(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    pin_data::pin_data(args.into(), input)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// Used to implement `PinnedDrop` safely.
///
/// Only works on structs that are annotated via `#[`[`macro@pin_data`]`]`.
///
/// # Examples
///
/// ```rust,ignore
/// #[pin_data(PinnedDrop)]
/// struct DriverData {
///     #[pin]
///     queue: Mutex<Vec<Command>>,
///     buf: Box<[u8; 1024 * 1024]>,
///     raw_info: *mut Info,
/// }
///
/// #[pinned_drop]
/// impl PinnedDrop for DriverData {
///     fn drop(self: Pin<&mut Self>) {
///         unsafe { bindings::destroy_info(self.raw_info) };
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn pinned_drop(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    parse_macro_input!(args as syn::parse::Nothing);
    pinned_drop::pinned_drop(input)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

/// Derives the [`Zeroable`] trait for the given struct.
///
/// This can only be used for structs where every field implements the [`Zeroable`] trait.
///
/// # Examples
///
/// ```rust,ignore
/// #[derive(Zeroable)]
/// pub struct DriverData {
///     id: i64,
///     buf_ptr: *mut u8,
///     len: usize,
/// }
/// ```
#[proc_macro_derive(Zeroable)]
pub fn derive_zeroable(input: TokenStream) -> TokenStream {
    let raw_input = input.clone();
    let input = parse_macro_input!(input as DeriveInput);
    zeroable::derive(input, raw_input.into())
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

// Documented in `pinned-init`.
#[proc_macro]
pub fn init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    init::init(input, true, false)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

// Documented in `pinned-init`.
#[proc_macro]
pub fn try_init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    init::init(input, false, false)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

// Documented in `pinned-init`.
#[proc_macro]
pub fn pin_init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    init::init(input, true, true)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}

// Documented in `pinned-init`.
#[proc_macro]
pub fn try_pin_init(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    init::init(input, false, true)
        .unwrap_or_else(|e| e.into_compile_error())
        .into()
}
