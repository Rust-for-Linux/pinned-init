// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Library to safely and fallibly initialize pinned `struct`s using in-place constructors.
//!
//! [Pinning][pinning] is Rust's way of ensuring data does not move.
//!
//! It also allows in-place initialization of big `struct`s that would otherwise produce a stack
//! overflow.
//!
//! This library's main use-case is in [Rust-for-Linux]. Although this version can be used
//! standalone.
//!
//! There are cases when you want to in-place initialize a struct. For example when it is very big
//! and moving it from the stack is not an option, because it is bigger than the stack itself.
//! Another reason would be that you need the address of the object to initialize it. This stands
//! in direct conflict with Rust's normal process of first initializing an object and then moving
//! it into it's final memory location.
//!
//! This library allows you to do in-place initialization safely.
//!
//! # Nightly only
//!
//! This library requires unstable features and thus can only be used with a nightly compiler.
//! The internally used features are:
//! - `allocator_api`
//! - `new_uninit` (only if the `alloc` or `std` features are enabled)
//! - `get_mut_unchecked` (only if the `alloc` or `std` features are enabled)
//!
//! The user will be required to activate these features:
//! - `allocator_api`
//!
//! # Overview
//!
//! To initialize a `struct` with an in-place constructor you will need two things:
//! - an in-place constructor,
//! - a memory location that can hold your `struct` (this can be the [stack], an [`Arc<T>`],
//!   [`Box<T>`] or any other smart pointer that implements [`InPlaceInit`]).
//!
//! To get an in-place constructor there are generally three options:
//! - directly creating an in-place constructor using the [`pin_init!`] macro,
//! - a custom function/macro returning an in-place constructor provided by someone else,
//! - using the unsafe function [`pin_init_from_closure()`] to manually create an initializer.
//!
//! Aside from pinned initialization, this library also supports in-place construction without pinning,
//! the macros/types/functions are generally named like the pinned variants without the `pin`
//! prefix.
//!
//! # Examples
//!
//! Throught some examples we will make use of the `CMutex` type which can be found in
//! `../examples/mutex.rs`. It is essentially a rebuild of the `mutex` from the Linux kernel in userland. So
//! it also uses a wait list and a basic spinlock. Importantly it needs to be pinned to be locked
//! and thus is a prime candidate for using this library.
//!
//! ## Using the [`pin_init!`] macro
//!
//! If you want to use [`PinInit`], then you will have to annotate your `struct` with
//! [`#[pin_data]`](pin_data). It is a macro that uses `#[pin]` as a marker for
//! [structurally pinned fields]. After doing this, you can then create an in-place constructor via
//! [`pin_init!`]. The syntax is almost the same as normal `struct` initializers. The difference is
//! that you need to write `<-` instead of `:` for fields that you want to initialize in-place.
//!
//! ```
//! # #![allow(clippy::disallowed_names)]
//! # #![feature(allocator_api)]
//! use pinned_init::{pin_data, pin_init};
//! # use core::pin::Pin;
//! # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
//! #[pin_data]
//! struct Foo {
//!     #[pin]
//!     a: CMutex<usize>,
//!     b: u32,
//! }
//!
//! let foo = pin_init!(Foo {
//!     a <- CMutex::new(42),
//!     b: 24,
//! });
//! # use pinned_init::InPlaceInit;
//! # let _ = Box::pin_init(foo);
//! ```
//!
//! `foo` now is of the type [`impl PinInit<Foo>`]. We can now use any smart pointer that we like
//! (or just the stack) to actually initialize a `Foo`:
//!
//! ```
//! # #![allow(clippy::disallowed_names)]
//! # #![feature(allocator_api)]
//! # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
//! # use pinned_init::{pin_data, pin_init};
//! # use core::pin::Pin;
//! # #[pin_data]
//! # struct Foo {
//! #     #[pin]
//! #     a: CMutex<usize>,
//! #     b: u32,
//! # }
//! # let foo = pin_init!(Foo {
//! #     a <- CMutex::new(42),
//! #     b: 24,
//! # });
//! use pinned_init::InPlaceInit;
//!
//! let foo: Result<Pin<Box<Foo>>, _> = Box::pin_init(foo);
//! ```
//!
//! For more information see the [`pin_init!`] macro.
//!
//! ## Using a custom function/macro that returns an initializer
//!
//! Many types that use this library supply a function/macro that returns an initializer, because
//! the above method only works for types where you can access the fields.
//!
//! ```
//! # #![feature(allocator_api)]
//! # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
//! # use std::sync::Arc;
//! # use core::pin::Pin;
//! use pinned_init::InPlaceInit;
//!
//! let mtx: Result<Pin<Arc<CMutex<usize>>>, _> = Arc::pin_init(CMutex::new(42));
//! ```
//!
//! To declare an init macro/function you just return an [`impl PinInit<T, E>`]:
//!
//! ```
//! # #![allow(clippy::disallowed_names)]
//! # #![feature(allocator_api)]
//! # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
//! use pinned_init::{pin_data, try_pin_init, PinInit, InPlaceInit};
//!
//! #[pin_data]
//! struct DriverData {
//!     #[pin]
//!     status: CMutex<i32>,
//!     buffer: Box<[u8; 1_000_000]>,
//! }
//!
//! impl DriverData {
//!     fn new() -> impl PinInit<Self, Error> {
//!         try_pin_init!(Self {
//!             status <- CMutex::new(0),
//!             buffer: Box::init(pinned_init::zeroed())?,
//!         }? Error)
//!     }
//! }
//! ```
//!
//! ## Manual creation of an initializer
//!
//! Often when working with primitives the previous approaches are not sufficient. That is where
//! [`pin_init_from_closure()`] comes in. This `unsafe` function allows you to create a
//! [`impl PinInit<T, E>`] directly from a closure. Of course you have to ensure that the closure
//! actually does the initialization in the correct way. Here are the things to look out for
//! (we are calling the parameter to the closure `slot`):
//! - when the closure returns `Ok(())`, then it has completed the initialization successfully, so
//!   `slot` now contains a valid bit pattern for the type `T`,
//! - when the closure returns `Err(e)`, then the caller may deallocate the memory at `slot`, so
//!   you need to take care to clean up anything if your initialization fails mid-way,
//! - you may assume that `slot` will stay pinned even after the closure returns until `drop` of
//!   `slot` gets called.
//!
//! ```
//! # #![feature(extern_types)]
//! use pinned_init::{pin_data, pinned_drop, pin_init_from_closure, PinInit};
//! use core::{ptr::addr_of_mut, marker::PhantomPinned, cell::UnsafeCell, pin::Pin};
//! mod bindings {
//!     extern "C" {
//!         pub type foo;
//!         pub fn init_foo(ptr: *mut foo);
//!         pub fn destroy_foo(ptr: *mut foo);
//!         #[must_use = "you must check the error return code"]
//!         pub fn enable_foo(ptr: *mut foo, flags: u32) -> i32;
//!     }
//! }
//!
//! /// # Invariants
//! ///
//! /// `foo` is always initialized
//! #[pin_data(PinnedDrop)]
//! pub struct RawFoo {
//!     #[pin]
//!     _p: PhantomPinned,
//!     #[pin]
//!     foo: UnsafeCell<bindings::foo>,
//! }
//!
//! impl RawFoo {
//!     pub fn new(flags: u32) -> impl PinInit<Self, i32> {
//!         // SAFETY:
//!         // - when the closure returns `Ok(())`, then it has successfully initialized and
//!         //   enabled `foo`,
//!         // - when it returns `Err(e)`, then it has cleaned up before
//!         unsafe {
//!             pin_init_from_closure(move |slot: *mut Self| {
//!                 // `slot` contains uninit memory, avoid creating a reference.
//!                 let foo = addr_of_mut!((*slot).foo);
//!
//!                 // Initialize the `foo`
//!                 bindings::init_foo(UnsafeCell::raw_get(foo));
//!
//!                 // Try to enable it.
//!                 let err = bindings::enable_foo(UnsafeCell::raw_get(foo), flags);
//!                 if err != 0 {
//!                     // Enabling has failed, first clean up the foo and then return the error.
//!                     bindings::destroy_foo(UnsafeCell::raw_get(foo));
//!                     Err(err)
//!                 } else {
//!                     // All fields of `RawFoo` have been initialized, since `_p` is a ZST.
//!                     Ok(())
//!                 }
//!             })
//!         }
//!     }
//! }
//!
//! #[pinned_drop]
//! impl PinnedDrop for RawFoo {
//!     fn drop(self: Pin<&mut Self>) {
//!         // SAFETY: Since `foo` is initialized, destroying is safe.
//!         unsafe { bindings::destroy_foo(self.foo.get()) };
//!     }
//! }
//! ```
//!
//! For more information on how to use [`pin_init_from_closure()`], take a look at the uses inside
//! the `kernel` crate. The [`sync`] module is a good starting point.
//!
//! [`sync`]: https://github.com/Rust-for-Linux/linux/tree/rust-next/rust/kernel/sync
//! [pinning]: https://doc.rust-lang.org/std/pin/index.html
//! [structurally pinned fields]:
//!     https://doc.rust-lang.org/std/pin/index.html#pinning-is-structural-for-field
//! [stack]: crate::stack_pin_init
//! [`Arc<T>`]: ::alloc::sync::Arc
//! [`Box<T>`]: ::alloc::boxed::Box
//! [`impl PinInit<Foo>`]: crate::PinInit
//! [`impl PinInit<T, E>`]: crate::PinInit
//! [`impl Init<T, E>`]: crate::Init
//! [Rust-for-Linux]: https://rust-for-linux.com/

#![forbid(missing_docs, unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(feature = "std"), no_std)]
#![feature(allocator_api)]
#![cfg_attr(any(feature = "alloc"), feature(new_uninit))]
#![cfg_attr(any(feature = "alloc"), feature(get_mut_unchecked))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::sync::Arc;

extern crate self as pinned_init;

use core::{
    alloc::AllocError,
    cell::UnsafeCell,
    convert::Infallible,
    marker::PhantomData,
    mem::MaybeUninit,
    num::*,
    pin::Pin,
    ptr::{self, NonNull},
};

#[doc(hidden)]
pub mod __internal;

/// Used to specify the pinning information of the fields of a struct.
///
/// This is somewhat similar in purpose as [pin-project](https://crates.io/crates/pin-project).
/// Place this macro on a struct definition and then `#[pin]` in on all fields you want to have
/// structurally pinned.
///
/// This macro enables the use of the [`pin_init!`] macro. When pinned-initializing a `struct`,
/// then `#[pin]` directs the type of initializer that is required.
///
/// # Implementing [`Drop`]
///
/// If you need to implement [`Drop`] for your `struct`, then you need to add `PinnedDrop` to the
/// arguments of this macro (so `#[pin_data(PinnedDrop)]`). Then you also need to implement
/// [`PinnedDrop`] annotated with [`#[pinned_drop]`](pinned_drop), since dropping pinned values
/// requires extra care.
///
/// # Examples
///
/// ## Normal Usage
///
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::pin_data;
///
/// #[pin_data]
/// struct CountedBuffer {
///     buf: [u8; 1024],
///     write_count: usize,
///     // Put `#[pin]` onto the fields that need structural pinning.
///     #[pin]
///     read_count: CMutex<usize>,
/// }
/// ```
///
/// ## With [`PinnedDrop`] Support
///
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::{pin_data, pinned_drop};
/// use std::pin::Pin;
///
/// #[pin_data(PinnedDrop)]
/// struct CountedBuffer {
///     buf: [u8; 1024],
///     write_count: usize,
///     // Put `#[pin]` onto the fields that need structural pinning.
///     #[pin]
///     read_count: CMutex<usize>,
/// }
///
/// #[pinned_drop]
/// impl PinnedDrop for CountedBuffer {
///     fn drop(self: Pin<&mut Self>) {
///         println!(
///             "CountedBuffer: written = {}, read = {}",
///             self.write_count,
///             *self.read_count.lock()
///         );
///     }
/// }
/// ```
///
pub use pinned_init_macro::pin_data;

/// Used to implement [`PinnedDrop`] safely.
///
/// Only works on implementations for [`PinnedDrop`] and only with structs that are annotated with
/// [`#[pin_data(PinnedDrop)]`](pin_data).
///
/// # Examples
///
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::{pin_data, pinned_drop};
/// use std::pin::Pin;
///
/// #[pin_data(PinnedDrop)]
/// struct CountedBuffer {
///     buf: [u8; 1024],
///     write_count: usize,
///     // Put `#[pin]` onto the fields that need structural pinning.
///     #[pin]
///     read_count: CMutex<usize>,
/// }
///
/// #[pinned_drop]
/// impl PinnedDrop for CountedBuffer {
///     fn drop(self: Pin<&mut Self>) {
///         println!(
///             "CountedBuffer: written = {}, read = {}",
///             self.write_count,
///             *self.read_count.lock()
///         );
///     }
/// }
/// ```
pub use pinned_init_macro::pinned_drop;

/// Derives the [`Zeroable`] trait for the given struct.
///
/// This can only be used for structs where every field implements the [`Zeroable`] trait.
///
/// # Examples
///
/// ```
/// use pinned_init::Zeroable;
///
/// #[derive(Zeroable)]
/// pub struct DriverData {
///     id: i64,
///     buf_ptr: *mut u8,
///     len: usize,
/// }
/// ```
///
/// You can also have generics, as the derive macro bounds them to implement the [`Zeroable`]
/// trait.
///
/// ```
/// use pinned_init::Zeroable;
///
/// #[derive(Zeroable)]
/// pub struct Data<T> {
///     value: T,
///     id: i64,
/// }
/// ```
pub use pinned_init_macro::Zeroable;

/// Initialize and pin a type directly on the stack.
///
/// # Examples
///
/// ```
/// # #![allow(clippy::disallowed_names)]
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// # use core::pin::Pin;
/// use pinned_init::{pin_data, stack_pin_init, pin_init};
///
/// #[pin_data]
/// struct Foo {
///     #[pin]
///     a: CMutex<usize>,
///     b: Bar,
/// }
///
/// #[pin_data]
/// struct Bar {
///     x: u32,
/// }
///
/// stack_pin_init!(let foo = pin_init!(Foo {
///     a <- CMutex::new(42),
///     b: Bar {
///         x: 64,
///     },
/// }));
/// let foo: Pin<&mut Foo> = foo;
/// println!("a: {}", &*foo.a.lock());
/// ```
///
/// # Syntax
///
/// A normal `let` binding with optional type annotation. The expression is expected to implement
/// [`PinInit`]/[`Init`] with the error type [`Infallible`]. If you want to use a different error
/// type, then use [`stack_try_pin_init!`].
#[macro_export]
macro_rules! stack_pin_init {
    (let $var:ident $(: $t:ty)? = $val:expr) => {
        let val = $val;
        let mut $var = ::core::pin::pin!($crate::__internal::StackInit$(::<$t>)?::uninit());
        let mut $var = match $crate::__internal::StackInit::init($var, val) {
            Ok(res) => res,
            Err(x) => {
                let x: ::core::convert::Infallible = x;
                match x {}
            }
        };
    };
}

/// Initialize and pin a type directly on the stack.
///
/// # Examples
///
/// ```
/// # #![allow(clippy::disallowed_names)]
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::{pin_data, stack_try_pin_init, try_pin_init};
///
/// #[pin_data]
/// struct Foo {
///     #[pin]
///     a: CMutex<usize>,
///     b: Box<Bar>,
/// }
///
/// struct Bar {
///     x: u32,
/// }
///
/// stack_try_pin_init!(let foo: Foo = try_pin_init!(Foo {
///     a <- CMutex::new(42),
///     b: Box::try_new(Bar {
///         x: 64,
///     })?,
/// }? Error));
/// let foo = foo.unwrap();
/// println!("a: {}", &*foo.a.lock());
/// ```
///
/// ```
/// # #![allow(clippy::disallowed_names)]
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::{pin_data, stack_try_pin_init, try_pin_init};
///
/// #[pin_data]
/// struct Foo {
///     #[pin]
///     a: CMutex<usize>,
///     b: Box<Bar>,
/// }
///
/// struct Bar {
///     x: u32,
/// }
///
/// stack_try_pin_init!(let foo: Foo =? try_pin_init!(Foo {
///     a <- CMutex::new(42),
///     b: Box::try_new(Bar {
///         x: 64,
///     })?,
/// }? Error));
/// println!("a: {}", &*foo.a.lock());
/// # Ok::<_, Error>(())
/// ```
///
/// # Syntax
///
/// A normal `let` binding with optional type annotation. The expression is expected to implement
/// [`PinInit`]/[`Init`]. This macro assigns a result to the given variable, adding a `?` after the
/// `=` will propagate this error.
#[macro_export]
macro_rules! stack_try_pin_init {
    (let $var:ident $(: $t:ty)? = $val:expr) => {
        let val = $val;
        let mut $var = ::core::pin::pin!($crate::__internal::StackInit$(::<$t>)?::uninit());
        let mut $var = $crate::__internal::StackInit::init($var, val);
    };
    (let $var:ident $(: $t:ty)? =? $val:expr) => {
        let val = $val;
        let mut $var = ::core::pin::pin!($crate::__internal::StackInit$(::<$t>)?::uninit());
        let mut $var = $crate::__internal::StackInit::init($var, val)?;
    };
}

/// Constructs an in-place initializer for `struct`s.
///
/// There are four variants of this macro:
/// - [`init!`],
/// - [`try_init!`],
/// - [`pin_init!`],
/// - [`try_pin_init!`].
///
/// The non-`try_` versions always result in an initializer that has [`Infallible`] as the error
/// type. When using the `try_` version, you have to specify the error manually. The `pin_` version
/// results in an initializer that ensures that the object will stay pinned. This makes it suitable
/// for initializing self referential objects.
///
/// **Note:** When using the `pin_` version, your struct *needs* to be annotated with the
/// [`#[pin_data]`](pin_data) macro.
///
/// # Examples
///
/// ## Embedding a Type that is Initialized by [`PinInit`]
///
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::{pin_data, pin_init, PinInit};
///
/// // Add `#[pin_data]` on your struct:
/// #[pin_data]
/// struct CountedBuffer {
///     buf: [u8; 1024],
///     write_count: usize,
///     // Put `#[pin]` onto the fields that are initialized by `PinInit`
///     #[pin]
///     read_count: CMutex<usize>,
/// }
///
/// impl CountedBuffer {
///     pub fn new(buf: [u8; 1024]) -> impl PinInit<Self> {
///         // Use the `pin_init!` macro when no error can occur during initialization.
///         pin_init!(Self {
///             // When initializing normal fields, just use the struct initializer syntax.
///             write_count: 0,
///
///             // You can also use the usual short-hand notation when there already is a variable
///             // with the name of the field:
///             buf, // This is equivalent to `buf: buf,`
///
///             // For fields that are initialized via `PinInit`, write `<-` instead of `:`.
///             read_count <- CMutex::new(0),
///         })
///     }
/// }
/// # use pinned_init::InPlaceInit;
/// # let _ = Box::pin_init(CountedBuffer::new([0; 1024]));
/// ```
///
/// ## Handling Failure in Initializers
///
/// When the initializer that you get is `impl PinInit<T, E>`, i.e. initialization can fail, you
/// cannot use [`pin_init!`] or [`init!`], instead use the respective `try_` variant:
/// [`try_pin_init!`] or [`try_init!`]. They also allow you to use the `?` operator and `return`
/// errors directly:
///
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::{pin_data, try_pin_init, PinInit, zeroed, InPlaceInit};
///
/// #[pin_data]
/// struct BigCountedBuf {
///     buf: Box<[u8; 1024 * 1024 * 1024]>,
///     write_count: usize,
///     #[pin]
///     read_count: CMutex<usize>,
/// }
///
/// impl BigCountedBuf {
///     pub fn new() -> impl PinInit<Self, Error> {
///         try_pin_init!(Self {
///             // When using `?`, it will return an error when the initializer runs, not when this
///             // function runs.
///             buf: Box::init(zeroed())?,
///             write_count: 0,
///             read_count <- CMutex::new(0),
///         }? Error)
///     }
/// }
/// ```
///
/// You need to specify the error type via `? $ty` after the normal initializer. Errors that are
/// returned inside of the initializer have to be compatible with the error at the end. The error
/// at the end needs to implement [`From`] the other errors.
///
/// When using an `impl PinInit<T, E>` via the `<-` syntax, you do not need to use the `?` operator:
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// # use pinned_init::{InPlaceInit, zeroed};
/// # #[pin_data]
/// # struct BigCountedBuf {
/// #     buf: Box<[u8; 1024 * 1024 * 1024]>,
/// #     write_count: usize,
/// #     #[pin]
/// #     read_count: CMutex<usize>,
/// # }
/// # impl BigCountedBuf {
/// #     pub fn new() -> impl PinInit<Self, Error> {
/// #         try_pin_init!(Self {
/// #             // When using `?`, it will return an error when the initializer runs, not when this
/// #             // function runs.
/// #             buf: Box::init(zeroed())?,
/// #             write_count: 0,
/// #             read_count <- CMutex::new(0),
/// #         }? Error)
/// #     }
/// # }
/// use pinned_init::{pin_data, try_pin_init, PinInit};
///
/// #[pin_data]
/// struct MultiBuf {
///     #[pin]
///     first: BigCountedBuf,
///     #[pin]
///     second: BigCountedBuf,
/// }
///
/// impl MultiBuf {
///     fn new() -> impl PinInit<Self, Error> {
///         try_pin_init!(Self {
///             // Notice that there is no `?` here.
///             first <- BigCountedBuf::new(),
///             second <- BigCountedBuf::new(),
///         }? Error)
///     }
/// }
/// ```
///
/// ## Defaulting the Value to Zero
///
/// Sometimes, structs have a lot of fields that should be initialized to zero. For this situation,
/// the initialization macros have special support for [`Zeroable`]. When your struct implement this
/// trait, you can write `..Zeroable::zeroed()` at the end of the initializer and all missing
/// fields will be initialized with zero:
///
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::{init, Zeroable, Init};
///
/// #[derive(Zeroable)]
/// struct Config {
///     max_size: usize,
///     port_a: u16,
///     port_b: u16,
///     port_c: u16,
///     port_d: u16,
///     // Imagine lot's more fields.
/// }
///
/// impl Config {
///     pub fn new() -> impl Init<Self> {
///         init!(Self {
///             max_size: 1024 * 1024,
///             port_b: 3000,
///             // This sets everything else to zero.
///             ..Zeroable::zeroed()
///         })
///     }
/// }
/// ```
///
/// ## Ensuring the Object stays Pinned
///
/// One important use case of this library is to ensure that an object is initialized in a pinned
/// state. To do this, you need to use [`PinInit`] and ensure that your struct is [`!Unpin`]. You
/// do this by adding a [`PhantomPinned`] field to your struct:
///
/// ```
/// use pinned_init::pin_data;
/// use core::marker::PhantomPinned;
///
/// #[pin_data]
/// struct MyStruct {
///     /* other fields */
///     // You also need to add `#[pin]`.
///     #[pin]
///     pin: PhantomPinned,
/// }
/// ```
///
/// ## Accessing the Memory Location of the Initialized Object
///
/// Since these initializers are in-place, there is a way to get a pointer to the location where it
/// is currently being initialized. To get such a pointer simply add `&this in` to the beginning of
/// the initializer:
///
/// ```
/// # use pinned_init::{pin_data, pin_init, PinInit};
/// # use core::{ptr::NonNull, marker::PhantomPinned};
/// # #[pin_data]
/// # pub struct ListLink {
/// #     next: NonNull<ListLink>,
/// #     prev: NonNull<ListLink>,
/// #     #[pin]
/// #     pin: PhantomPinned,
/// # }
/// # impl ListLink {
/// #     pub fn new() -> impl PinInit<Self> {
/// pin_init!(&this in Self {
///     // ...
/// #    next: this,
/// #    prev: this,
/// #    pin: PhantomPinned,
/// })
/// #     }
/// # }
/// ```
///
/// The type of `this` is [`NonNull<Self>`]. Here is an example of how to use it:
///
/// ```
/// use pinned_init::{pin_data, pin_init, PinInit};
/// use core::{ptr::NonNull, marker::PhantomPinned};
///
/// #[pin_data]
/// pub struct ListLink {
///     next: NonNull<ListLink>,
///     prev: NonNull<ListLink>,
///     #[pin]
///     pin: PhantomPinned,
/// }
///
/// impl ListLink {
///     pub fn new() -> impl PinInit<Self> {
///         pin_init!(&this in Self {
///             next: this,
///             prev: this,
///             pin: PhantomPinned,
///         })
///     }
/// }
/// ```
///
/// **Note:** your struct must be [`!Unpin`] and you need to use [`PinInit`] when you want to use
/// the pointer after the initializer has finished. Otherwise it is not guaranteed that the object
/// stays at the initial location.
///
/// # Advanced Information
///
/// ## Initialization Order
///
/// The fields are initialized in the order that they appear in the initializer. When
/// initialization fails for one of the fields, all previously initialized fields will be dropped
/// in reverse order.
///
/// ## Syntax
///
/// Here you can see the syntax in a pseudo rust macro format:
/// - `$()?` means the contents of the `()` are optional,
/// - `$(),*` means the contents can be repeated arbitrarily with `,` separating them without a
///   required trailing `,`,
/// - `$|` means that either the left side or the right side can be matched (this is not official
///   syntax in rust macros),
/// - `$ident` matches any identifier,
/// - `$expr` any expression,
/// - `$path` any path,
/// - `$ty` any type.
///
/// ```
/// # /*
/// $(&$ident in)? $path {
///     $(
///         $ident <- $expr
///       $|
///         $ident $(: $expr)?
///     ),*
///     $(
///         ,
///         $(..Zeroable::zeroed())?
///     )?
/// }? $ty
/// # */
/// ```
///
/// When using a non-`try_` version, then the last part with `? $ty` is not expected.
///
/// [`NonNull<Self>`]: core::ptr::NonNull
/// [`!Unpin`]: core::marker::Unpin
/// [`PhantomPinned`]: core::marker::PhantomPinned
pub use pinned_init_macro::{init, pin_init, try_init, try_pin_init};

/// A pin-initializer for the type `T`.
///
/// To use this initializer, you will need a suitable memory location that can hold a `T`. This can
/// be [`Box<T>`], [`Arc<T>`] or even the stack (see [`stack_pin_init!`]). Use the
/// [`InPlaceInit::try_pin_init`] function of a smart pointer like [`Arc<T>`] on this.
///
/// Also see the [module description](self).
///
/// # Safety
///
/// When implementing this trait you will need to take great care. Also there are probably very few
/// cases where a manual implementation is necessary. Use [`pin_init_from_closure`] where possible.
///
/// The [`PinInit::__pinned_init`] function:
/// - returns `Ok(())` if it initialized every field of `slot`,
/// - returns `Err(err)` if it encountered an error and then cleaned `slot`, this means:
///     - `slot` can be deallocated without UB occurring,
///     - `slot` does not need to be dropped,
///     - `slot` is not partially initialized.
/// - while constructing the `T` at `slot` it upholds the pinning invariants of `T`.
///
/// [`Arc<T>`]: alloc::sync::Arc
#[must_use = "An initializer must be used in order to create its value."]
pub unsafe trait PinInit<T: ?Sized, E = Infallible>: Sized {
    /// Initializes `slot`.
    ///
    /// # Safety
    ///
    /// - `slot` is a valid pointer to uninitialized memory.
    /// - the caller does not touch `slot` when `Err` is returned, they are only permitted to
    ///   deallocate.
    /// - `slot` will not move until it is dropped, i.e. it will be pinned.
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E>;

    /// First initializes the value using `self` then calls the function `f` with the initialized
    /// value.
    ///
    /// If `f` returns an error the value is dropped and the initializer will forward the error.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![feature(allocator_api)]
    /// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
    /// use pinned_init::PinInit;
    ///
    /// let mtx_init = CMutex::new(42);
    /// // Make the initializer print the value.
    /// let mtx_init = mtx_init.pin_chain(|mtx| {
    ///     println!("{:?}", mtx.get_data_mut());
    ///     Ok(())
    /// });
    /// ```
    fn pin_chain<F>(self, f: F) -> impl PinInit<T, E>
    where
        F: FnOnce(Pin<&mut T>) -> Result<(), E>,
    {
        __internal::ChainPinInit(self, f, PhantomData)
    }
}

/// An initializer for `T`.
///
/// To use this initializer, you will need a suitable memory location that can hold a `T`. This can
/// be [`Box<T>`], [`Arc<T>`] or even the stack (see [`stack_pin_init!`]). Use the
/// [`InPlaceInit::try_init`] function of a smart pointer like [`Arc<T>`] on this. Because
/// [`PinInit<T, E>`] is a super trait, you can use every function that takes it as well.
///
/// Also see the [module description](self).
///
/// # Safety
///
/// When implementing this trait you will need to take great care. Also there are probably very few
/// cases where a manual implementation is necessary. Use [`init_from_closure`] where possible.
///
/// The [`Init::__init`] function:
/// - returns `Ok(())` if it initialized every field of `slot`,
/// - returns `Err(err)` if it encountered an error and then cleaned `slot`, this means:
///     - `slot` can be deallocated without UB occurring,
///     - `slot` does not need to be dropped,
///     - `slot` is not partially initialized.
/// - while constructing the `T` at `slot` it upholds the pinning invariants of `T`.
///
/// The `__pinned_init` function from the supertrait [`PinInit`] needs to execute the exact same
/// code as `__init`.
///
/// Contrary to its supertype [`PinInit<T, E>`] the caller is allowed to
/// move the pointee after initialization.
///
/// [`Arc<T>`]: alloc::sync::Arc
#[must_use = "An initializer must be used in order to create its value."]
pub unsafe trait Init<T: ?Sized, E = Infallible>: PinInit<T, E> {
    /// Initializes `slot`.
    ///
    /// # Safety
    ///
    /// - `slot` is a valid pointer to uninitialized memory.
    /// - the caller does not touch `slot` when `Err` is returned, they are only permitted to
    ///   deallocate.
    unsafe fn __init(self, slot: *mut T) -> Result<(), E>;

    /// First initializes the value using `self` then calls the function `f` with the initialized
    /// value.
    ///
    /// If `f` returns an error the value is dropped and the initializer will forward the error.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(clippy::disallowed_names)]
    /// use pinned_init::{init, zeroed, Init};
    ///
    /// struct Foo {
    ///     buf: [u8; 1_000_000],
    /// }
    ///
    /// impl Foo {
    ///     fn setup(&mut self) {
    ///         println!("Setting up foo");
    ///     }
    /// }
    ///
    /// let foo = init!(Foo {
    ///     buf <- zeroed()
    /// }).chain(|foo| {
    ///     foo.setup();
    ///     Ok(())
    /// });
    /// ```
    fn chain<F>(self, f: F) -> impl Init<T, E>
    where
        F: FnOnce(&mut T) -> Result<(), E>,
    {
        __internal::ChainInit(self, f, PhantomData)
    }
}

/// Creates a new [`PinInit<T, E>`] from the given closure.
///
/// # Safety
///
/// The closure:
/// - returns `Ok(())` if it initialized every field of `slot`,
/// - returns `Err(err)` if it encountered an error and then cleaned `slot`, this means:
///     - `slot` can be deallocated without UB occurring,
///     - `slot` does not need to be dropped,
///     - `slot` is not partially initialized.
/// - may assume that the `slot` does not move if `T: !Unpin`,
/// - while constructing the `T` at `slot` it upholds the pinning invariants of `T`.
#[inline]
pub const unsafe fn pin_init_from_closure<T: ?Sized, E>(
    f: impl FnOnce(*mut T) -> Result<(), E>,
) -> impl PinInit<T, E> {
    __internal::InitClosure(f, PhantomData)
}

/// Creates a new [`Init<T, E>`] from the given closure.
///
/// # Safety
///
/// The closure:
/// - returns `Ok(())` if it initialized every field of `slot`,
/// - returns `Err(err)` if it encountered an error and then cleaned `slot`, this means:
///     - `slot` can be deallocated without UB occurring,
///     - `slot` does not need to be dropped,
///     - `slot` is not partially initialized.
/// - the `slot` may move after initialization.
/// - while constructing the `T` at `slot` it upholds the pinning invariants of `T`.
#[inline]
pub const unsafe fn init_from_closure<T: ?Sized, E>(
    f: impl FnOnce(*mut T) -> Result<(), E>,
) -> impl Init<T, E> {
    __internal::InitClosure(f, PhantomData)
}

/// An initializer that leaves the memory uninitialized.
///
/// The initializer is a no-op. The `slot` memory is not changed.
#[inline]
pub fn uninit<T, E>() -> impl Init<MaybeUninit<T>, E> {
    // SAFETY: The memory is allowed to be uninitialized.
    unsafe { init_from_closure(|_| Ok(())) }
}

/// Initializes an array by initializing each element via the provided initializer.
///
/// # Examples
///
/// ```
/// use pinned_init::{init_array_from_fn, InPlaceInit};
///
/// let array: Box<[usize; 1_000]> = Box::init(init_array_from_fn(|i| i)).unwrap();
/// assert_eq!(array.len(), 1_000);
/// ```
pub fn init_array_from_fn<I, const N: usize, T, E>(
    mut make_init: impl FnMut(usize) -> I,
) -> impl Init<[T; N], E>
where
    I: Init<T, E>,
{
    let init = move |slot: *mut [T; N]| {
        let slot = slot.cast::<T>();
        for i in 0..N {
            let init = make_init(i);
            // SAFETY: Since 0 <= `i` < N, it is still in bounds of `[T; N]`.
            let ptr = unsafe { slot.add(i) };
            // SAFETY: The pointer is derived from `slot` and thus satisfies the `__init`
            // requirements.
            match unsafe { init.__init(ptr) } {
                Ok(()) => {}
                Err(e) => {
                    // SAFETY: The loop has initialized the elements `slot[0..i]` and since we
                    // return `Err` below, `slot` will be considered uninitialized memory.
                    unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(slot, i)) };
                    return Err(e);
                }
            }
        }
        Ok(())
    };
    // SAFETY: The initializer above initializes every element of the array. On failure it drops
    // any initialized elements and returns `Err`.
    unsafe { init_from_closure(init) }
}

/// Initializes an array by initializing each element via the provided initializer.
///
/// # Examples
///
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// # use core::pin::Pin;
/// use pinned_init::{pin_init_array_from_fn, InPlaceInit};
/// use std::sync::Arc;
///
/// let array: Pin<Arc<[CMutex<usize>; 1_000]>> =
///     Arc::pin_init(pin_init_array_from_fn(|i| CMutex::new(i))).unwrap();
/// assert_eq!(array.len(), 1_000);
/// ```
pub fn pin_init_array_from_fn<I, const N: usize, T, E>(
    mut make_init: impl FnMut(usize) -> I,
) -> impl PinInit<[T; N], E>
where
    I: PinInit<T, E>,
{
    let init = move |slot: *mut [T; N]| {
        let slot = slot.cast::<T>();
        for i in 0..N {
            let init = make_init(i);
            // SAFETY: Since 0 <= `i` < N, it is still in bounds of `[T; N]`.
            let ptr = unsafe { slot.add(i) };
            // SAFETY: The pointer is derived from `slot` and thus satisfies the `__init`
            // requirements.
            match unsafe { init.__pinned_init(ptr) } {
                Ok(()) => {}
                Err(e) => {
                    // SAFETY: The loop has initialized the elements `slot[0..i]` and since we
                    // return `Err` below, `slot` will be considered uninitialized memory.
                    unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(slot, i)) };
                    return Err(e);
                }
            }
        }
        Ok(())
    };
    // SAFETY: The initializer above initializes every element of the array. On failure it drops
    // any initialized elements and returns `Err`.
    unsafe { pin_init_from_closure(init) }
}

// SAFETY: Every type can be initialized by-value.
unsafe impl<T, E> Init<T, E> for T {
    unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
        unsafe { slot.write(self) };
        Ok(())
    }
}

// SAFETY: Every type can be initialized by-value. `__pinned_init` calls `__init`.
unsafe impl<T, E> PinInit<T, E> for T {
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        unsafe { self.__init(slot) }
    }
}

/// Smart pointer that can initialize memory in-place.
pub trait InPlaceInit<T>: Sized {
    /// Use the given pin-initializer to pin-initialize a `T` inside of a new smart pointer of this
    /// type.
    ///
    /// If `T: !Unpin` it will not be able to move afterwards.
    fn try_pin_init<E>(init: impl PinInit<T, E>) -> Result<Pin<Self>, E>
    where
        E: From<AllocError>;

    /// Use the given pin-initializer to pin-initialize a `T` inside of a new smart pointer of this
    /// type.
    ///
    /// If `T: !Unpin` it will not be able to move afterwards.
    fn pin_init(init: impl PinInit<T>) -> Result<Pin<Self>, AllocError> {
        // SAFETY: We delegate to `init` and only change the error type.
        let init = unsafe {
            pin_init_from_closure(|slot| match init.__pinned_init(slot) {
                Ok(()) => Ok(()),
                Err(i) => match i {},
            })
        };
        Self::try_pin_init(init)
    }

    /// Use the given initializer to in-place initialize a `T`.
    fn try_init<E>(init: impl Init<T, E>) -> Result<Self, E>
    where
        E: From<AllocError>;

    /// Use the given initializer to in-place initialize a `T`.
    fn init(init: impl Init<T>) -> Result<Self, AllocError> {
        // SAFETY: We delegate to `init` and only change the error type.
        let init = unsafe {
            init_from_closure(|slot| match init.__init(slot) {
                Ok(()) => Ok(()),
                Err(i) => match i {},
            })
        };
        Self::try_init(init)
    }
}

#[cfg(feature = "alloc")]
impl<T> InPlaceInit<T> for Box<T> {
    #[inline]
    fn try_pin_init<E>(init: impl PinInit<T, E>) -> Result<Pin<Self>, E>
    where
        E: From<AllocError>,
    {
        let mut this = Box::try_new_uninit()?;
        let slot = this.as_mut_ptr();
        // SAFETY: When init errors/panics, slot will get deallocated but not dropped,
        // slot is valid and will not be moved, because we pin it later.
        unsafe { init.__pinned_init(slot)? };
        // SAFETY: All fields have been initialized.
        Ok(unsafe { this.assume_init() }.into())
    }

    #[inline]
    fn try_init<E>(init: impl Init<T, E>) -> Result<Self, E>
    where
        E: From<AllocError>,
    {
        let mut this = Box::try_new_uninit()?;
        let slot = this.as_mut_ptr();
        // SAFETY: When init errors/panics, slot will get deallocated but not dropped,
        // slot is valid.
        unsafe { init.__init(slot)? };
        // SAFETY: All fields have been initialized.
        Ok(unsafe { this.assume_init() })
    }
}

#[cfg(feature = "alloc")]
impl<T> InPlaceInit<T> for Arc<T> {
    #[inline]
    fn try_pin_init<E>(init: impl PinInit<T, E>) -> Result<Pin<Self>, E>
    where
        E: From<AllocError>,
    {
        let mut this = Arc::try_new_uninit()?;
        let slot = unsafe { Arc::get_mut_unchecked(&mut this) };
        let slot = slot.as_mut_ptr();
        // SAFETY: When init errors/panics, slot will get deallocated but not dropped,
        // slot is valid and will not be moved, because we pin it later.
        unsafe { init.__pinned_init(slot)? };
        // SAFETY: All fields have been initialized and this is the only `Arc` to that data.
        Ok(unsafe { Pin::new_unchecked(this.assume_init()) })
    }

    #[inline]
    fn try_init<E>(init: impl Init<T, E>) -> Result<Self, E>
    where
        E: From<AllocError>,
    {
        let mut this = Arc::try_new_uninit()?;
        let slot = unsafe { Arc::get_mut_unchecked(&mut this) };
        let slot = slot.as_mut_ptr();
        // SAFETY: When init errors/panics, slot will get deallocated but not dropped,
        // slot is valid.
        unsafe { init.__init(slot)? };
        // SAFETY: All fields have been initialized.
        Ok(unsafe { this.assume_init() })
    }
}

/// Trait facilitating pinned destruction.
///
/// Use [`#[pinned_drop]`](pinned_drop) to implement this trait safely:
///
/// ```
/// # #![feature(allocator_api)]
/// # #[path = "../examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::{pin_data, pinned_drop};
/// use core::pin::Pin;
///
/// #[pin_data(PinnedDrop)]
/// struct Foo {
///     #[pin]
///     mtx: CMutex<usize>,
/// }
///
/// #[pinned_drop]
/// impl PinnedDrop for Foo {
///     fn drop(self: Pin<&mut Self>) {
///         println!("Foo is being dropped!");
///     }
/// }
/// ```
///
/// # Safety
///
/// This trait must be implemented via the [`#[pinned_drop]`](pinned_drop) proc-macro attribute on
/// the impl.
pub unsafe trait PinnedDrop: __internal::HasPinData {
    /// Executes the pinned destructor of this type.
    ///
    /// While this function is marked safe, it is actually unsafe to call it manually. For this
    /// reason it takes an additional parameter. This type can only be constructed by `unsafe` code
    /// and thus prevents this function from being called where it should not.
    ///
    /// This extra parameter will be generated by the [`#[pinned_drop]`](pinned_drop) proc-macro
    /// attribute automatically.
    fn drop(self: Pin<&mut Self>, only_call_from_drop: __internal::OnlyCallFromDrop);
}

/// Marker trait for types that can be initialized by writing just zeroes.
///
/// The common use-case of this trait is the [`zeroed()`] function. Or using the
/// `..Zeroable::zeroed()` syntax of the [`[try_][pin_]init!`](pin_init) macros.
///
/// Easily and safely implement this trait using the [`macro@Zeroable`] derive macro.
///
/// # Safety
///
/// The bit pattern consisting of only zeroes is a valid bit pattern for this type. In other words,
/// this is not UB:
///
/// ```
/// # struct S; impl S { fn create() -> Self {
/// let val: Self = unsafe { core::mem::zeroed() };
/// # val } }
/// ```
pub unsafe trait Zeroable {}

/// Create a new zeroed T.
///
/// The returned initializer will write `0x00` to every byte of the given `slot`.
#[inline]
pub fn zeroed<T: Zeroable>() -> impl Init<T> {
    // SAFETY: Because `T: Zeroable`, all bytes zero is a valid bit pattern for `T`
    // and because we write all zeroes, the memory is initialized.
    unsafe {
        init_from_closure(|slot: *mut T| {
            slot.write_bytes(0, 1);
            Ok(())
        })
    }
}

macro_rules! impl_zeroable {
    ($($(#[$attr:meta])*$({$($generics:tt)*})? $t:ty, )*) => {
        $(
            $(#[$attr])*
            unsafe impl$($($generics)*)? Zeroable for $t {}
        )*
    };
}

impl_zeroable! {
    // SAFETY: All primitives that are allowed to be zero.
    bool,
    char,
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
    f32, f64,

    // Note: do not add uninhabited types (such as `!` or `core::convert::Infallible`) to this list;
    // creating an instance of an uninhabited type is immediate undefined behavior. For more on
    // uninhabited/empty types, consult The Rustonomicon:
    // <https://doc.rust-lang.org/stable/nomicon/exotic-sizes.html#empty-types>. The Rust Reference
    // also has information on undefined behavior:
    // <https://doc.rust-lang.org/stable/reference/behavior-considered-undefined.html>.
    //
    // SAFETY: These are inhabited ZSTs; there is nothing to zero and a valid value exists.
    {<T: ?Sized>} PhantomData<T>, core::marker::PhantomPinned, (),

    // SAFETY: Type is allowed to take any value, including all zeros.
    {<T>} MaybeUninit<T>,

    // SAFETY: `T: Zeroable` and `UnsafeCell` is `repr(transparent)`.
    {<T: ?Sized + Zeroable>} UnsafeCell<T>,

    // SAFETY: All zeros is equivalent to `None` (option layout optimization guarantee).
    Option<NonZeroU8>, Option<NonZeroU16>, Option<NonZeroU32>, Option<NonZeroU64>,
    Option<NonZeroU128>, Option<NonZeroUsize>,
    Option<NonZeroI8>, Option<NonZeroI16>, Option<NonZeroI32>, Option<NonZeroI64>,
    Option<NonZeroI128>, Option<NonZeroIsize>,

    // SAFETY: All zeros is equivalent to `None` (option layout optimization guarantee).
    //
    // In this case we are allowed to use `T: ?Sized`, since all zeros is the `None` variant.
    {<T: ?Sized>} Option<NonNull<T>>,
    #[cfg(feature = "alloc")]
    {<T: ?Sized>} Option<Box<T>>,

    // SAFETY: `null` pointer is valid.
    //
    // We cannot use `T: ?Sized`, since the VTABLE pointer part of fat pointers is not allowed to be
    // null.
    //
    // When `Pointee` gets stabilized, we could use
    // `T: ?Sized where <T as Pointee>::Metadata: Zeroable`
    {<T>} *mut T, {<T>} *const T,

    // SAFETY: `null` pointer is valid and the metadata part of these fat pointers is allowed to be
    // zero.
    {<T>} *mut [T], {<T>} *const [T], *mut str, *const str,

    // SAFETY: `T` is `Zeroable`.
    {<const N: usize, T: Zeroable>} [T; N], {<T: Zeroable>} Wrapping<T>,
}

macro_rules! impl_tuple_zeroable {
    ($(,)?) => {};
    ($first:ident, $($t:ident),* $(,)?) => {
        // SAFETY: All elements are zeroable and padding can be zero.
        unsafe impl<$first: Zeroable, $($t: Zeroable),*> Zeroable for ($first, $($t),*) {}
        impl_tuple_zeroable!($($t),* ,);
    }
}

impl_tuple_zeroable!(A, B, C, D, E, F, G, H, I, J);
