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
//! `./examples/mutex.rs`. It is essentially a rebuild of the `mutex` from the Linux kernel in userland. So
//! it also uses a wait list and a basic spinlock. Importantly it needs to be pinned to be locked
//! and thus is a prime candidate for using this library.
//!
//! ## Using the [`pin_init!`] macro
//!
//! If you want to use [`PinInit`], then you will have to annotate your `struct` with
//! `#[`[`pin_data`]`]`. It is a macro that uses `#[pin]` as a marker for
//! [structurally pinned fields]. After doing this, you can then create an in-place constructor via
//! [`pin_init!`]. The syntax is almost the same as normal `struct` initializers. The difference is
//! that you need to write `<-` instead of `:` for fields that you want to initialize in-place.
//!
//! ```rust
//! # #![allow(clippy::disallowed_names)]
//! # #![feature(allocator_api)]
//! use pinned_init::*;
//! # use core::pin::Pin;
//! # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
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
//! # let _ = Box::pin_init(foo);
//! ```
//!
//! `foo` now is of the type [`impl PinInit<Foo>`]. We can now use any smart pointer that we like
//! (or just the stack) to actually initialize a `Foo`:
//!
//! ```rust
//! # #![allow(clippy::disallowed_names)]
//! # #![feature(allocator_api)]
//! # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
//! # use pinned_init::*;
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
//! ```rust
//! # #![feature(allocator_api)]
//! # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
//! # use pinned_init::*;
//! # use std::sync::Arc;
//! # use core::pin::Pin;
//! let mtx: Result<Pin<Arc<CMutex<usize>>>, _> = Arc::pin_init(CMutex::new(42));
//! ```
//!
//! To declare an init macro/function you just return an [`impl PinInit<T, E>`]:
//!
//! ```rust
//! # #![allow(clippy::disallowed_names)]
//! # #![feature(allocator_api)]
//! # use pinned_init::*;
//! # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
//! #[pin_data]
//! struct DriverData {
//!     #[pin]
//!     status: CMutex<i32>,
//!     buffer: Box<[u8; 1_000_000]>,
//! }
//!
//! impl DriverData {
//!     fn new() -> impl PinInit<Self, AllocError> {
//!         try_pin_init!(Self {
//!             status <- CMutex::new(0),
//!             buffer: Box::init(pinned_init::zeroed())?,
//!         }? AllocError)
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
//! ```rust
//! # #![feature(extern_types)]
//! use pinned_init::*;
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
#![cfg_attr(kernel, doc = "[`Arc<T>`]: ../kernel/sync/struct.Arc.html")]
#![cfg_attr(not(kernel), doc = "[`Arc<T>`]: alloc::sync::Arc")]
//! [`Box<T>`]: alloc::boxed::Box
//! [`impl PinInit<Foo>`]: PinInit
//! [`impl PinInit<T, E>`]: PinInit
//! [`impl Init<T, E>`]: Init
//! [Rust-for-Linux]: https://rust-for-linux.com/
//! [`pin_init!`]: crate::pin_init!

#![forbid(missing_docs, unsafe_op_in_unsafe_fn)]
#![cfg_attr(any(not(feature = "std"), kernel), no_std)]
#![feature(allocator_api)]
#![cfg_attr(any(feature = "alloc", kernel), feature(new_uninit))]
#![cfg_attr(feature = "alloc", feature(get_mut_unchecked))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(any(feature = "alloc", kernel))]
use alloc::boxed::Box;
#[cfg(feature = "alloc")]
use alloc::sync::Arc;
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
#[doc(hidden)]
pub mod macros;

#[cfg(not(kernel))]
pub use pinned_init_macro::{pin_data, pinned_drop, Zeroable};

#[cfg(kernel)]
pub use ::macros::{pin_data, pinned_drop, Zeroable};

#[allow(unused_extern_crates)]
extern crate self as pinned_init;

/// Initialize and pin a type directly on the stack.
///
/// # Examples
///
/// ```rust
/// # #![allow(clippy::disallowed_names)]
/// # #![feature(allocator_api)]
/// # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
/// # use pinned_init::*;
/// # use core::pin::Pin;
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
///
/// [`stack_try_pin_init!`]: crate::stack_try_pin_init!
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
/// ```rust
/// # #![allow(clippy::disallowed_names)]
/// # #![feature(allocator_api)]
/// # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
/// # use pinned_init::*;
/// # use core::pin::Pin;
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
/// }? AllocError));
/// let foo = foo.unwrap();
/// println!("a: {}", &*foo.a.lock());
/// ```
///
/// ```rust
/// # #![allow(clippy::disallowed_names)]
/// # #![feature(allocator_api)]
/// # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
/// # use pinned_init::*;
/// # use core::pin::Pin;
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
/// }? AllocError));
/// println!("a: {}", &*foo.a.lock());
/// # Ok::<_, AllocError>(())
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

/// Construct an in-place, pinned initializer for `struct`s.
///
/// This macro defaults the error to [`Infallible`]. If you need a different error, then use
/// [`try_pin_init!`].
///
/// The syntax is almost identical to that of a normal `struct` initializer:
///
/// ```rust
/// # #![allow(clippy::disallowed_names)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// #[pin_data]
/// struct Foo {
///     a: usize,
///     b: Bar,
/// }
///
/// #[pin_data]
/// struct Bar {
///     x: u32,
/// }
///
/// # fn demo() -> impl PinInit<Foo> {
/// let a = 42;
///
/// let initializer = pin_init!(Foo {
///     a,
///     b: Bar {
///         x: 64,
///     },
/// });
/// # initializer }
/// # Box::pin_init(demo()).unwrap();
/// ```
///
/// Arbitrary Rust expressions can be used to set the value of a variable.
///
/// The fields are initialized in the order that they appear in the initializer. So it is possible
/// to read already initialized fields using raw pointers.
///
/// IMPORTANT: You are not allowed to create references to fields of the struct inside of the
/// initializer.
///
/// # Init-functions
///
/// When working with this library it is often desired to let others construct your types without
/// giving access to all fields. This is where you would normally write a plain function `new`
/// that would return a new instance of your type. With this library that is also possible.
/// However, there are a few extra things to keep in mind.
///
/// To create an initializer function, simply declare it like this:
///
/// ```rust
/// # #![allow(clippy::disallowed_names)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # #[pin_data]
/// # struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }
/// # #[pin_data]
/// # struct Bar {
/// #     x: u32,
/// # }
/// impl Foo {
///     fn new() -> impl PinInit<Self> {
///         pin_init!(Self {
///             a: 42,
///             b: Bar {
///                 x: 64,
///             },
///         })
///     }
/// }
/// ```
///
/// Users of `Foo` can now create it like this:
///
/// ```rust
/// # #![allow(clippy::disallowed_names)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # #[pin_data]
/// # struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }
/// # #[pin_data]
/// # struct Bar {
/// #     x: u32,
/// # }
/// # impl Foo {
/// #     fn new() -> impl PinInit<Self> {
/// #         pin_init!(Self {
/// #             a: 42,
/// #             b: Bar {
/// #                 x: 64,
/// #             },
/// #         })
/// #     }
/// # }
/// let foo = Box::pin_init(Foo::new());
/// ```
///
/// They can also easily embed it into their own `struct`s:
///
/// ```rust
/// # #![allow(clippy::disallowed_names)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # #[pin_data]
/// # struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }
/// # #[pin_data]
/// # struct Bar {
/// #     x: u32,
/// # }
/// # impl Foo {
/// #     fn new() -> impl PinInit<Self> {
/// #         pin_init!(Self {
/// #             a: 42,
/// #             b: Bar {
/// #                 x: 64,
/// #             },
/// #         })
/// #     }
/// # }
/// #[pin_data]
/// struct FooContainer {
///     #[pin]
///     foo1: Foo,
///     #[pin]
///     foo2: Foo,
///     other: u32,
/// }
///
/// impl FooContainer {
///     fn new(other: u32) -> impl PinInit<Self> {
///         pin_init!(Self {
///             foo1 <- Foo::new(),
///             foo2 <- Foo::new(),
///             other,
///         })
///     }
/// }
/// ```
///
/// Here we see that when using `pin_init!` with `PinInit`, one needs to write `<-` instead of `:`.
/// This signifies that the given field is initialized in-place. As with `struct` initializers, just
/// writing the field (in this case `other`) without `:` or `<-` means `other: other,`.
///
/// # Syntax
///
/// As already mentioned in the examples above, inside of `pin_init!` a `struct` initializer with
/// the following modifications is expected:
/// - Fields that you want to initialize in-place have to use `<-` instead of `:`.
/// - In front of the initializer you can write `&this in` to have access to a [`NonNull<Self>`]
///   pointer named `this` inside of the initializer.
/// - Using struct update syntax one can place `..Zeroable::zeroed()` at the very end of the
///   struct, this initializes every field with 0 and then runs all initializers specified in the
///   body. This can only be done if [`Zeroable`] is implemented for the struct.
///
/// For instance:
///
/// ```rust
/// # use pinned_init::*;
/// # use core::{ptr::addr_of_mut, marker::PhantomPinned};
/// #[pin_data]
/// #[derive(Zeroable)]
/// struct Buf {
///     // `ptr` points into `buf`.
///     ptr: *mut u8,
///     buf: [u8; 64],
///     #[pin]
///     pin: PhantomPinned,
/// }
/// pin_init!(&this in Buf {
///     buf: [0; 64],
///     ptr: unsafe { addr_of_mut!((*this.as_ptr()).buf).cast() },
///     pin: PhantomPinned,
/// });
/// pin_init!(Buf {
///     buf: [1; 64],
///     ..Zeroable::zeroed()
/// });
/// ```
///
/// [`try_pin_init!`]: crate::try_pin_init
/// [`NonNull<Self>`]: core::ptr::NonNull
// For a detailed example of how this macro works, see the module documentation of the hidden
// module `__internal` inside of `./__internal.rs`.
#[macro_export]
macro_rules! pin_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::try_pin_init!($(&$this in)? $t $(::<$($generics),* $(,)?>)? {
            $($fields)*
        }? ::core::convert::Infallible)
    };
}

/// Construct an in-place, fallible pinned initializer for `struct`s.
///
/// If the initialization can complete without error (or [`Infallible`]), then use [`pin_init!`].
///
/// You can use the `?` operator or use `return Err(err)` inside the initializer to stop
/// initialization and return the error.
///
/// IMPORTANT: if you have `unsafe` code inside of the initializer you have to ensure that when
/// initialization fails, the memory can be safely deallocated without any further modifications.
///
/// The syntax is identical to [`pin_init!`] with the following exception: you must append `? $type`
/// after the `struct` initializer to specify the error type you want to use.
///
/// # Examples
///
/// ```rust
/// # #![feature(allocator_api, new_uninit)]
/// # use core::alloc::AllocError;
/// use pinned_init::*;
/// #[pin_data]
/// struct BigBuf {
///     big: Box<[u8; 1024 * 1024 * 1024]>,
///     small: [u8; 1024 * 1024],
///     ptr: *mut u8,
/// }
///
/// impl BigBuf {
///     fn new() -> impl PinInit<Self, AllocError> {
///         try_pin_init!(Self {
///             big: Box::init(zeroed())?,
///             small: [0; 1024 * 1024],
///             ptr: core::ptr::null_mut(),
///         }? AllocError)
///     }
/// }
/// # let _ = Box::pin_init(BigBuf::new());
/// ```
// For a detailed example of how this macro works, see the module documentation of the hidden
// module `__internal` inside of `./__internal.rs`.
#[macro_export]
macro_rules! try_pin_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }? $err:ty) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)? ),
            @fields($($fields)*),
            @error($err),
            @data(PinData, use_data),
            @has_data(HasPinData, __pin_data),
            @construct_closure(pin_init_from_closure),
            @munch_fields($($fields)*),
        )
    }
}

/// Construct an in-place initializer for `struct`s.
///
/// This macro defaults the error to [`Infallible`]. If you need a different error, then use
/// [`try_init!`].
///
/// The syntax is identical to [`pin_init!`] and its safety caveats also apply:
/// - `unsafe` code must guarantee either full initialization or return an error and allow
///   deallocation of the memory.
/// - the fields are initialized in the order given in the initializer.
/// - no references to fields are allowed to be created inside of the initializer.
///
/// This initializer is for initializing data in-place that might later be moved. If you want to
/// pin-initialize, use [`pin_init!`].
///
/// # Examples
///
/// ```rust
/// # #![feature(allocator_api)]
/// # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
/// use pinned_init::*;
/// struct BigBuf {
///     big: Box<[u8; 1024 * 1024 * 1024]>,
///     small: [u8; 1024 * 1024],
/// }
///
/// impl BigBuf {
///     fn new() -> impl Init<Self, AllocError> {
///         try_init!(Self {
///             small <- zeroed(),
///             big: Box::init(zeroed())?,
///         }? AllocError)
///     }
/// }
/// # let _ = Box::init(BigBuf::new());
/// ```
///
/// [`try_init!`]: crate::try_init!
// For a detailed example of how this macro works, see the module documentation of the hidden
// module `__internal` inside of `./__internal.rs`.
#[macro_export]
macro_rules! init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }) => {
        $crate::try_init!($(&$this in)? $t $(::<$($generics),* $(,)?>)? {
            $($fields)*
        }? ::core::convert::Infallible)
    }
}

/// Construct an in-place fallible initializer for `struct`s.
///
/// If the initialization can complete without error (or [`Infallible`]), then use
/// [`init!`].
///
/// The syntax is identical to [`try_pin_init!`]. You need to specify a custom error
/// via `? $type` after the `struct` initializer.
/// The safety caveats from [`try_pin_init!`] also apply:
/// - `unsafe` code must guarantee either full initialization or return an error and allow
///   deallocation of the memory.
/// - the fields are initialized in the order given in the initializer.
/// - no references to fields are allowed to be created inside of the initializer.
///
/// # Examples
///
/// ```rust
/// # #![feature(allocator_api)]
/// # use core::alloc::AllocError;
/// use pinned_init::*;
/// struct BigBuf {
///     big: Box<[u8; 1024 * 1024 * 1024]>,
///     small: [u8; 1024 * 1024],
/// }
///
/// impl BigBuf {
///     fn new() -> impl Init<Self, AllocError> {
///         try_init!(Self {
///             big: Box::init(zeroed())?,
///             small: [0; 1024 * 1024],
///         }? AllocError)
///     }
/// }
/// # let _ = Box::init(BigBuf::new());
/// ```
// For a detailed example of how this macro works, see the module documentation of the hidden
// module `__internal` inside of `./__internal.rs`.
#[macro_export]
macro_rules! try_init {
    ($(&$this:ident in)? $t:ident $(::<$($generics:ty),* $(,)?>)? {
        $($fields:tt)*
    }? $err:ty) => {
        $crate::__init_internal!(
            @this($($this)?),
            @typ($t $(::<$($generics),*>)?),
            @fields($($fields)*),
            @error($err),
            @data(InitData, /*no use_data*/),
            @has_data(HasInitData, __init_data),
            @construct_closure(init_from_closure),
            @munch_fields($($fields)*),
        )
    }
}

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
#[cfg_attr(kernel, doc = "[`Arc<T>`]: ../kernel/sync/struct.Arc.html")]
#[cfg_attr(not(kernel), doc = "[`Arc<T>`]: alloc::sync::Arc")]
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
    /// ```rust
    /// # #![feature(allocator_api)]
    /// # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
    /// # use pinned_init::*;
    /// let mtx_init = CMutex::new(42);
    /// // Make the initializer print the value.
    /// let mtx_init = mtx_init.pin_chain(|mtx| {
    ///     println!("{:?}", mtx.get_data_mut());
    ///     Ok(())
    /// });
    /// ```
    fn pin_chain<F>(self, f: F) -> ChainPinInit<Self, F, T, E>
    where
        F: FnOnce(Pin<&mut T>) -> Result<(), E>,
    {
        ChainPinInit(self, f, PhantomData)
    }
}

/// An initializer returned by [`PinInit::pin_chain`].
pub struct ChainPinInit<I, F, T: ?Sized, E>(I, F, __internal::Invariant<(E, Box<T>)>);

// SAFETY: The `__pinned_init` function is implemented such that it
// - returns `Ok(())` on successful initialization,
// - returns `Err(err)` on error and in this case `slot` will be dropped.
// - considers `slot` pinned.
unsafe impl<T: ?Sized, E, I, F> PinInit<T, E> for ChainPinInit<I, F, T, E>
where
    I: PinInit<T, E>,
    F: FnOnce(Pin<&mut T>) -> Result<(), E>,
{
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        // SAFETY: All requirements fulfilled since this function is `__pinned_init`.
        unsafe { self.0.__pinned_init(slot)? };
        // SAFETY: The above call initialized `slot` and we still have unique access.
        let val = unsafe { &mut *slot };
        // SAFETY: `slot` is considered pinned.
        let val = unsafe { Pin::new_unchecked(val) };
        (self.1)(val).map_err(|e| {
            // SAFETY: `slot` was initialized above.
            unsafe { core::ptr::drop_in_place(slot) };
            e
        })
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
#[cfg_attr(kernel, doc = "[`Arc<T>`]: ../kernel/sync/struct.Arc.html")]
#[cfg_attr(not(kernel), doc = "[`Arc<T>`]: alloc::sync::Arc")]
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
    /// ```rust
    /// # #![allow(clippy::disallowed_names)]
    /// # use pinned_init::*;
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
    ///     buf <- pinned_init::zeroed()
    /// }).chain(|foo| {
    ///     foo.setup();
    ///     Ok(())
    /// });
    /// ```
    fn chain<F>(self, f: F) -> ChainInit<Self, F, T, E>
    where
        F: FnOnce(&mut T) -> Result<(), E>,
    {
        ChainInit(self, f, PhantomData)
    }
}

/// An initializer returned by [`Init::chain`].
pub struct ChainInit<I, F, T: ?Sized, E>(I, F, __internal::Invariant<(E, Box<T>)>);

// SAFETY: The `__init` function is implemented such that it
// - returns `Ok(())` on successful initialization,
// - returns `Err(err)` on error and in this case `slot` will be dropped.
unsafe impl<T: ?Sized, E, I, F> Init<T, E> for ChainInit<I, F, T, E>
where
    I: Init<T, E>,
    F: FnOnce(&mut T) -> Result<(), E>,
{
    unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
        // SAFETY: All requirements fulfilled since this function is `__init`.
        unsafe { self.0.__pinned_init(slot)? };
        // SAFETY: The above call initialized `slot` and we still have unique access.
        (self.1)(unsafe { &mut *slot }).map_err(|e| {
            // SAFETY: `slot` was initialized above.
            unsafe { core::ptr::drop_in_place(slot) };
            e
        })
    }
}

// SAFETY: `__pinned_init` behaves exactly the same as `__init`.
unsafe impl<T: ?Sized, E, I, F> PinInit<T, E> for ChainInit<I, F, T, E>
where
    I: Init<T, E>,
    F: FnOnce(&mut T) -> Result<(), E>,
{
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        // SAFETY: `__init` has less strict requirements compared to `__pinned_init`.
        unsafe { self.__init(slot) }
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
/// ```rust
/// # use pinned_init::*;
/// let array: Box<[usize; 1_000]> = Box::init::<Error>(init_array_from_fn(|i| i)).unwrap();
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
/// ```rust
/// # #![feature(allocator_api)]
/// # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
/// # use pinned_init::*;
/// # use std::sync::Arc;
/// let array: Arc<[CMutex<usize>; 1_000]> =
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
    #[cfg(not(kernel))]
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
    #[cfg(not(kernel))]
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

#[cfg(any(feature = "alloc", kernel))]
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
        // SAFETY: when init errors/panics, slot will get deallocated but not dropped,
        // slot is valid.
        unsafe { init.__init(slot)? };
        // SAFETY: All fields have been initialized.
        Ok(unsafe { this.assume_init() })
    }
}

/// Trait facilitating pinned destruction.
///
/// Use [`pinned_drop`] to implement this trait safely:
///
/// ```rust
/// # #![feature(allocator_api)]
/// # #[path = "./examples/mutex.rs"] mod mutex; use mutex::*;
/// # use pinned_init::*;
/// use core::pin::Pin;
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
/// This trait must be implemented via the [`pinned_drop`] proc-macro attribute on the impl.
pub unsafe trait PinnedDrop: __internal::HasPinData {
    /// Executes the pinned destructor of this type.
    ///
    /// While this function is marked safe, it is actually unsafe to call it manually. For this
    /// reason it takes an additional parameter. This type can only be constructed by `unsafe` code
    /// and thus prevents this function from being called where it should not.
    ///
    /// This extra parameter will be generated by the `#[pinned_drop]` proc-macro attribute
    /// automatically.
    fn drop(self: Pin<&mut Self>, only_call_from_drop: __internal::OnlyCallFromDrop);
}

/// Marker trait for types that can be initialized by writing just zeroes.
///
/// # Safety
///
/// The bit pattern consisting of only zeroes is a valid bit pattern for this type. In other words,
/// this is not UB:
///
/// ```rust,ignore
/// let val: Self = unsafe { core::mem::zeroed() };
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
    ($($(#[$attr:meta])* $({$($generics:tt)*})? $t:ty, )*) => {
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

    // SAFETY: These are ZSTs, there is nothing to zero.
    {<T: ?Sized>} PhantomData<T>, core::marker::PhantomPinned, Infallible, (),

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
    #[cfg(any(feature = "alloc", kernel))]
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
