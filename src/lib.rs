#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
//
#![feature(never_type)]
#![feature(allocator_api)]
#![cfg_attr(
    any(feature = "alloc", feature = "std"),
    feature(new_uninit, get_mut_unchecked)
)]
#![doc = include_str!("../README.md")]

#[cfg(feature = "alloc")]
use alloc::alloc::AllocError;
use core::{marker::PhantomData, mem::MaybeUninit, pin::Pin, ptr};
#[cfg(feature = "std")]
use std::alloc::AllocError;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::{boxed::Box, rc::Rc, sync::Arc};
#[cfg(all(not(feature = "alloc"), feature = "std"))]
use std::{boxed::Box, rc::Rc, sync::Arc};

#[doc(hidden)]
pub mod __private;

#[cfg(doctest)]
mod tests;

/// Initialize a type on the stack. It will be pinned:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// pin_data! {
///     struct Foo {
///         a: usize,
///         b: Bar,
///     }
/// }
///
/// pin_data! {
///     struct Bar {
///         x: u32,
///     }
/// }
///
/// let a = 42;
///
/// stack_init!(let foo = pin_init!(Foo {
///     a,
///     b: Bar {
///         x: 64,
///     },
/// }));
/// let foo: Result<Pin<&mut Foo>, !> = foo;
/// ```
#[macro_export]
macro_rules! stack_init {
    (let $var:ident = $val:expr) => {
        let mut $var = $crate::__private::StackInit::uninit();
        let val = $val;
        let mut $var = unsafe { $crate::__private::StackInit::init(&mut $var, val) };
    };
}

/// Construct an in-place initializer for structs.
///
/// The syntax is identical to a normal struct initializer:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// pin_data! {
///     struct Foo {
///         a: usize,
///         b: Bar,
///     }
/// }
///
/// pin_data! {
///     struct Bar {
///         x: u32,
///     }
/// }
///
/// let a = 42;
///
/// let initializer = pin_init!(Foo {
///     a,
///     b: Bar {
///         x: 64,
///     },
/// });
/// # let _: Result<Pin<Box<Foo>>, AllocOrInitError<!>> = Box::pin_init(initializer);
/// ```
/// Arbitrary rust expressions can be used to set the value of a variable.
///
/// # Init-functions
///
/// When working with this library it is often desired to let others construct your types without
/// giving access to all fields. This is where you would normally write a plain function `new`
/// that would return a new instance of your type. With this library that is also possible, however
/// there are a few extra things to keep in mind.
///
/// To create an initializer function, simple declare it like this:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # pin_data! { struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }}
/// # pin_data! { struct Bar {
/// #     x: u32,
/// # }}
///
/// impl Foo {
///     pub fn new() -> impl PinInit<Self, !> {
///         pin_init!(Self {
///             a: 42,
///             b: Bar {
///                 x: 64,
///             },
///         })
///     }
/// }
/// ```
/// Users of `Foo` can now create it like this:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # pin_data! { struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }}
/// # pin_data! { struct Bar {
/// #     x: u32,
/// # }}
/// # impl Foo {
/// #     pub fn new() -> impl PinInit<Self, !> {
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
/// They can also easily embed it into their own `struct`s:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # pin_data! { struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }}
/// # pin_data! { struct Bar {
/// #     x: u32,
/// # }}
/// # impl Foo {
/// #     pub fn new() -> impl PinInit<Self, !> {
/// #         pin_init!(Self {
/// #             a: 42,
/// #             b: Bar {
/// #                 x: 64,
/// #             },
/// #         })
/// #     }
/// # }
/// pin_data! {
///     struct FooContainer {
///         #[pin]
///         foo1: Foo,
///         #[pin]
///         foo2: Foo,
///         other: u32,
///     }
/// }
///
/// impl FooContainer {
///     pub fn new(other: u32) -> impl PinInit<Self, !> {
///         pin_init!(Self {
///             foo1: Foo::new(),
///             foo2: Foo::new(),
///             other,
///         })
///     }
/// }
/// ```
#[macro_export]
macro_rules! pin_init {
    ($(&$this:ident in)? $t:ident $(<$($generics:ty),* $(,)?>)? {
        $($field:ident $(: $val:expr)?),*
        $(,)?
    }) => {
        $crate::pin_init!(@this($($this)?), @type_name($t $($($generics),*)?), @typ($t $($($generics),*)?), @fields($($field $(: $val)?),*));
    };
    (@this($($this:ident)?), @type_name($t:ident $(<$($generics:ty),*)?), @typ($ty:ty), @fields($($field:ident $(: $val:expr)?),*)) => {{
        // we do not want to allow arbitrary returns
        struct __InitOk;
        let init = move |slot: *mut $ty| -> ::core::result::Result<__InitOk, _> {
            {
                // shadow the structure so it cannot be used to return early
                struct __InitOk;
                $(let $this = unsafe { ::core::ptr::NonNull::new_unchecked(slot) };)?
                $(
                    $(let $field = $val;)?
                    // call the initializer
                    // SAFETY: slot is valid, because we are inside of an initializer closure, we return
                    //         when an error/panic occurs.
                    unsafe {
                        <$ty as $crate::__private::__PinData>::__PinData::$field(
                            ::core::ptr::addr_of_mut!((*slot).$field),
                            $field,
                        )?;
                    }
                    // create the drop guard
                    // SAFETY: we forget the guard later when initialization has succeeded.
                    let $field = unsafe { $crate::__private::DropGuard::new(::core::ptr::addr_of_mut!((*slot).$field)) };
                )*
                #[allow(unreachable_code, clippy::diverging_sub_expression)]
                if false {
                    let _: $t $(<$($generics),*>)? = $t {
                        $($field: ::core::todo!()),*
                    };
                }
                $(
                    ::core::mem::forget($field);
                )*
            }
            Ok(__InitOk)
        };
        let init = move |slot: *mut $ty| -> ::core::result::Result<(), _> {
            init(slot).map(|__InitOk| ())
        };
        let init: $crate::PinInitClosure<_, $t $(<$($generics),*>)?, _> = unsafe { $crate::PinInitClosure::from_closure(init) };
        init
    }}
}

/// Construct an in-place initializer for structs.
///
/// The syntax is identical to a normal struct initializer:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// struct Foo {
///     a: usize,
///     b: Bar,
/// }
///
/// struct Bar {
///     x: u32,
/// }
///
/// let a = 42;
///
/// let initializer = init!(Foo {
///     a,
///     b: Bar {
///         x: 64,
///     },
/// });
/// # let _: Result<Box<Foo>, AllocOrInitError<!>> = Box::init(initializer);
/// ```
/// Arbitrary rust expressions can be used to set the value of a variable.
///
/// # Init-functions
///
/// When working with this library it is often desired to let others construct your types without
/// giving access to all fields. This is where you would normally write a plain function `new`
/// that would return a new instance of your type. With this library that is also possible, however
/// there are a few extra things to keep in mind.
///
/// To create an initializer function, simple declare it like this:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }
/// # struct Bar {
/// #     x: u32,
/// # }
///
/// impl Foo {
///     pub fn new() -> impl Init<Self, !> {
///         init!(Self {
///             a: 42,
///             b: Bar {
///                 x: 64,
///             },
///         })
///     }
/// }
/// ```
/// Users of `Foo` can now create it like this:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }
/// # struct Bar {
/// #     x: u32,
/// # }
/// # impl Foo {
/// #     pub fn new() -> impl Init<Self, !> {
/// #         init!(Self {
/// #             a: 42,
/// #             b: Bar {
/// #                 x: 64,
/// #             },
/// #         })
/// #     }
/// # }
/// let foo = Box::init(Foo::new());
/// ```
/// They can also easily embed it into their own `struct`s:
/// ```rust
/// # #![feature(never_type)]
/// # use pinned_init::*;
/// # use core::pin::Pin;
/// # struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }
/// # struct Bar {
/// #     x: u32,
/// # }
/// # impl Foo {
/// #     pub fn new() -> impl Init<Self, !> {
/// #         init!(Self {
/// #             a: 42,
/// #             b: Bar {
/// #                 x: 64,
/// #             },
/// #         })
/// #     }
/// # }
/// struct FooContainer {
///     foo1: Foo,
///     foo2: Foo,
///     other: u32,
/// }
///
/// impl FooContainer {
///     pub fn new(other: u32) -> impl Init<Self, !> {
///         init!(Self {
///             foo1: Foo::new(),
///             foo2: Foo::new(),
///             other,
///         })
///     }
/// }
/// ```
#[macro_export]
macro_rules! init {
    ($t:ident $(<$($generics:ty),* $(,)?>)? {
        $($field:ident $(: $val:expr)?),*
        $(,)?
    }) => {{
        // we do not want to allow arbitrary returns
        struct __InitOk;
        let init = move |slot: *mut $t $(<$($generics),*>)?| -> ::core::result::Result<__InitOk, _> {
            {
                // shadow the structure so it cannot be used to return early
                struct __InitOk;
                $(
                    $(let $field = $val;)?
                    // call the initializer
                    // SAFETY: slot is valid, because we are inside of an initializer closure, we return
                    //         when an error/panic occurs.
                    unsafe { $crate::__private::__InitImpl::__init($field, ::core::ptr::addr_of_mut!((*slot).$field))? };
                    // create the drop guard
                    // SAFETY: we forget the guard later when initialization has succeeded.
                    let $field = unsafe { $crate::__private::DropGuard::new(::core::ptr::addr_of_mut!((*slot).$field)) };
                )*
                #[allow(unreachable_code, clippy::diverging_sub_expression)]
                if false {
                    let _: $t $(<$($generics),*>)? = $t {
                        $($field: ::core::todo!()),*
                    };
                }
                $(
                    // forget each guard
                    ::core::mem::forget($field);
                )*
            }
            Ok(__InitOk)
        };
        let init = move |slot: *mut $t $(<$($generics),*>)?| -> ::core::result::Result<(), _> {
            init(slot).map(|__InitOk| ())
        };
        let init: $crate::InitClosure<_, $t $(<$($generics),*>)?, _> = unsafe { $crate::InitClosure::from_closure(init) };
        init
    }}
}

/// Used to specify the pin information of the fields of a struct.
///
/// This is somewhat similar in purpose as
/// [pin-project-lite](https://crates.io/crates/pin-project-lite).
/// Place this macro around a struct definition and then `#[pin]` in front of the attributes of each
/// field you want to have structurally pinned.
///
/// needed to use `pin_init`.
#[macro_export]
macro_rules! pin_data {
    (@make_fn(($vis:vis) (#[pin] $(#[$attr:meta])*) $field:ident : $typ:ty)) => {
        $vis unsafe fn $field<E, W: $crate::__private::InitWay>(
            slot: *mut $typ,
            init: impl $crate::__private::__PinInitImpl<$typ, E, W>,
        ) -> ::core::result::Result<(), E> {
            $crate::__private::__PinInitImpl::__pinned_init(init, slot)
        }
    };
    (@make_fn(($vis:vis) () $field:ident : $typ:ty)) => {
        $vis unsafe fn $field<E, W: $crate::__private::InitWay>(
            slot: *mut $typ,
            init: impl $crate::__private::__InitImpl<$typ, E, W>,
        ) -> ::core::result::Result<(), E> {
            $crate::__private::__InitImpl::__init(init, slot)
        }
    };
    (@make_fn(($vis:vis) (#[$next:meta] $(#[$attr:meta])*) $field:ident : $typ:ty)) => {
        $crate::pin_data!(@make_fn(($vis) ($(#[$attr])*) $field: $typ));
    };
    (@filter(
        ($($pre:tt)*)
        {
            #[pin]
            $(#[$attr:meta])*
            $fvis:vis $field:ident : $typ:ty,
            $($rest:tt)*
        }
        ($($accum:tt)*)
    )) => {
        $crate::pin_data!(@filter(
            ($($pre)*)
            {
                $(#[$attr])*
                $fvis $field: $typ,
                $($rest)*
            }
            ($($accum)*)
        ));
    };
    (@filter(
        ($($pre:tt)*)
        {
            #[$next:meta]
            $(#[$attr:meta])*
            $fvis:vis $field:ident : $typ:ty,
            $($rest:tt)*
        }
        ($($accum:tt)*)
    )) => {
        $crate::pin_data!(@filter(
            ($($pre)*)
            {
                $(#[$attr])*
                $fvis $field: $typ,
                $($rest)*
            }
            ($($accum)* #[$next])
        ));
    };
    (@filter(
        ($($pre:tt)*)
        {
            $fvis:vis $field:ident : $typ:ty,
            $($rest:tt)*
        }
        ($($accum:tt)*)
    )) => {
        $crate::pin_data!(@filter(
            ($($pre)*)
            {
                $($rest)*
            }
            ($($accum)* $fvis $field: $typ,)
        ));
    };
    (@filter(
        ($($pre:tt)*)
        {}
        ($($accum:tt)*)
    )) => {
        $($pre)* {
            $($accum)*
        }
    };
    (
        $(#[$struct_attr:meta])*
        $vis:vis struct $name:ident $(<$($($life:lifetime),+ $(,)?)? $($generic:ident $(: ?$qbound:ty)?),* $(,)?>)?
        $(where $($whr:path : $bound:ty),* $(,)?)? {
            $(
                $(#[$($attr:tt)*])*
                $fvis:vis $field:ident : $typ:ty
            ),*
            $(,)?
        }
    ) => {
        $crate::pin_data!(@filter(
            (
                $(#[$struct_attr])*
                $vis struct $name $(<$($($life),+ ,)? $($generic $(: ?$qbound)?),*>)? $(where $($whr : $bound),*)?
            )
            {
                $(
                    $(#[$($attr)*])*
                    $fvis $field: $typ,
                )*
            }
            ()
        ));

        const _: () = {
            #[doc(hidden)]
            $vis struct __ThePinData$(<$($($life),+ ,)? $($generic $(: ?$qbound)?),*>)? $(where $($whr : $bound),*)?
                (::core::marker::PhantomData<fn($name$(<$($($life),+ ,)? $($generic),*>)?) -> $name$(<$($($life),+ ,)? $($generic),*>)?>);

            impl$(<$($($life),+ ,)? $($generic $(: ?$qbound)?),*>)? __ThePinData$(<$($($life),+ ,)? $($generic),*>)?
            $(where $($whr : $bound),*)? {
                $(
                    $crate::pin_data!(@make_fn(($fvis) ($(#[$($attr)*])*) $field: $typ));
                )*
            }

            unsafe impl$(<$($($life),+ ,)? $($generic $(: ?$qbound)?),*>)? $crate::__private::__PinData for $name$(<$($($life),+ ,)? $($generic),*>)?
            $(where $($whr : $bound),*)? {
                type __PinData = __ThePinData$(<$($($life),+ ,)? $($generic),*>)?;
            }
        };
    };
}

/// An initializer for `T`.
///
/// # Safety
/// The [`PinInit::__pinned_init`] function
/// - returns `Ok(())` iff it initialized every field of slot,
/// - returns `Err(err)` iff it encountered an error and then cleaned slot, this means:
///     - slot can be deallocated without UB ocurring,
///     - slot does not need to be dropped,
///     - slot is not partially initialized.
pub unsafe trait PinInit<T, E = !>: Sized {
    /// Initializes `slot`.
    ///
    /// # Safety
    /// `slot` is a valid pointer to uninitialized memory.
    /// The caller does not touch `slot` when `Err` is returned, they are only permitted to
    /// deallocate.
    /// The slot will not move, i.e. it will be pinned.
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E>;
}

/// An initializer for `T`.
///
/// # Safety
/// The [`Init::__init`] function
/// - returns `Ok(())` iff it initialized every field of slot,
/// - returns `Err(err)` iff it encountered an error and then cleaned slot, this means:
///     - slot can be deallocated without UB ocurring,
///     - slot does not need to be dropped,
///     - slot is not partially initialized.
///
/// The `__pinned_init` function from the supertrait [`PinInit`] needs to exectute the exact same
/// code as `__init`.
///
/// Contrary to its supertype [`PinInit<T, E>`] the caller is allowed to
/// move the pointee after initialization.
pub unsafe trait Init<T, E = !>: PinInit<T, E> {
    /// Initializes `slot`.
    ///
    /// # Safety
    /// `slot` is a valid pointer to uninitialized memory.
    /// The caller does not touch `slot` when `Err` is returned, they are only permitted to
    /// deallocate.
    unsafe fn __init(self, slot: *mut T) -> Result<(), E>;
}

type Invariant<T> = PhantomData<fn(T) -> T>;

/// A closure initializer.
pub struct InitClosure<F, T, E>(F, Invariant<(T, E)>);

impl<T, E, F> InitClosure<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    /// Creates a new Init from the given closure
    ///
    /// # Safety
    /// The closure
    /// - returns `Ok(())` iff it initialized every field of slot,
    /// - returns `Err(err)` iff it encountered an error and then cleaned slot, this means:
    ///     - slot can be deallocated without UB ocurring,
    ///     - slot does not need to be dropped,
    ///     - slot is not partially initialized.
    /// - slot may move after initialization
    pub const unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

unsafe impl<T, F, E> PinInit<T, E> for InitClosure<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        (self.0)(slot)
    }
}

unsafe impl<T, F, E> Init<T, E> for InitClosure<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
        (self.0)(slot)
    }
}

/// A closure initializer for pinned data.
pub struct PinInitClosure<F, T, E>(F, Invariant<(T, E)>);

impl<T, E, F> PinInitClosure<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    /// Creates a new Init from the given closure
    ///
    /// # Safety
    /// The closure
    /// - returns `Ok(())` iff it initialized every field of slot,
    /// - returns `Err(err)` iff it encountered an error and then cleaned slot, this means:
    ///     - slot can be deallocated without UB ocurring,
    ///     - slot does not need to be dropped,
    ///     - slot is not partially initialized.
    pub const unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

unsafe impl<T, F, E> PinInit<T, E> for PinInitClosure<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        (self.0)(slot)
    }
}

/// Smart pointer that can initialize memory in-place.
pub trait InPlaceInit<T>: Sized {
    /// The error that can occur when creating this pointer
    type Error<E>;

    /// Use the given initializer to in-place initialize a `T`.
    ///
    /// If `T: !Unpin` it will not be able to move afterwards.
    fn pin_init<E>(init: impl PinInit<T, E>) -> Result<Pin<Self>, Self::Error<E>>;

    /// Use the given initializer to in-place initialize a `T`.
    fn init<E>(init: impl Init<T, E>) -> Result<Self, Self::Error<E>>;
}

/// Either an allocation error, or an initialization error.
#[derive(Debug)]
pub enum AllocOrInitError<E> {
    /// An error from initializing a value
    Init(E),
    /// An error from trying to allocate memory
    Alloc,
}

impl<E> From<!> for AllocOrInitError<E> {
    fn from(e: !) -> Self {
        match e {}
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<E> From<AllocError> for AllocOrInitError<E> {
    fn from(_: AllocError) -> Self {
        Self::Alloc
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<T> InPlaceInit<T> for Box<T> {
    type Error<E> = AllocOrInitError<E>;

    fn pin_init<E>(init: impl PinInit<T, E>) -> Result<Pin<Self>, Self::Error<E>> {
        let mut this = Box::try_new_uninit()?;
        let slot = this.as_mut_ptr();
        // SAFETY: when init errors/panics, slot will get deallocated but not dropped,
        // slot is valid and will not be moved because of the into_pin
        unsafe { init.__pinned_init(slot).map_err(AllocOrInitError::Init)? };
        // SAFETY: all fields have been initialized
        Ok(Box::into_pin(unsafe { this.assume_init() }))
    }

    fn init<E>(init: impl Init<T, E>) -> Result<Self, Self::Error<E>> {
        let mut this = Box::try_new_uninit()?;
        let slot = this.as_mut_ptr();
        // SAFETY: when init errors/panics, slot will get deallocated but not dropped,
        // slot is valid
        unsafe { init.__init(slot).map_err(AllocOrInitError::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { this.assume_init() })
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<T> InPlaceInit<T> for Arc<T> {
    type Error<E> = AllocOrInitError<E>;

    fn pin_init<E>(init: impl PinInit<T, E>) -> Result<Pin<Self>, Self::Error<E>> {
        let mut this = Arc::try_new_uninit()?;
        let slot = unsafe { Arc::get_mut_unchecked(&mut this) }.as_mut_ptr();
        // SAFETY: when init errors/panics, slot will get deallocated but not dropped,
        // slot is valid and will not be moved because of the into_pin
        unsafe { init.__pinned_init(slot).map_err(AllocOrInitError::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { Pin::new_unchecked(this.assume_init()) })
    }

    fn init<E>(init: impl Init<T, E>) -> Result<Self, Self::Error<E>> {
        let mut this = Arc::try_new_uninit()?;
        let slot = unsafe { Arc::get_mut_unchecked(&mut this) }.as_mut_ptr();
        // SAFETY: when init errors/panics, slot will get deallocated but not dropped,
        // slot is valid
        unsafe { init.__init(slot).map_err(AllocOrInitError::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { this.assume_init() })
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<T> InPlaceInit<T> for Rc<T> {
    type Error<E> = AllocOrInitError<E>;

    fn pin_init<E>(init: impl PinInit<T, E>) -> Result<Pin<Self>, Self::Error<E>> {
        let mut this = Rc::try_new_uninit()?;
        let slot = unsafe { Rc::get_mut_unchecked(&mut this) }.as_mut_ptr();
        // SAFETY: when init errors/panics, slot will get deallocated but not dropped,
        // slot is valid and will not be moved because of the into_pin
        unsafe { init.__pinned_init(slot).map_err(AllocOrInitError::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { Pin::new_unchecked(this.assume_init()) })
    }

    fn init<E>(init: impl Init<T, E>) -> Result<Self, Self::Error<E>> {
        let mut this = Rc::try_new_uninit()?;
        let slot = unsafe { Rc::get_mut_unchecked(&mut this) }.as_mut_ptr();
        // SAFETY: when init errors/panics, slot will get deallocated but not dropped,
        // slot is valid
        unsafe { init.__init(slot).map_err(AllocOrInitError::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { this.assume_init() })
    }
}

/// Marker trait for types that can be initialized by writing just zeroes.
///
/// # Safety
/// The bit pattern consisting of only zeroes must be a valid bit pattern for the type.
pub unsafe trait Zeroable {}

/// Create a new zeroed T
pub fn zeroed<T: Zeroable>() -> impl Init<T, !> {
    unsafe {
        InitClosure::from_closure(|slot: *mut T| {
            slot.write_bytes(0, 1);
            Ok(())
        })
    }
}

/// An initializer that leaves the memory uninitialized.
pub fn uninit<T>() -> impl Init<MaybeUninit<T>, !> {
    unsafe { InitClosure::from_closure(|_| Ok(())) }
}

macro_rules! impl_zeroable {
    ($($t:ty),*) => {
        $(unsafe impl Zeroable for $t {})*
    };
}
// all primitives that are allowed to be 0
impl_zeroable!(
    bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);
// there is nothing to zero
impl_zeroable!(core::marker::PhantomPinned, !, ());

// we are allowed to zero padding bytes
unsafe impl<const N: usize, T: Zeroable> Zeroable for [T; N] {}

// there is nothing to zero
unsafe impl<T: ?Sized> Zeroable for PhantomData<T> {}

// null pointer is valid
unsafe impl<T: ?Sized> Zeroable for *mut T {}
unsafe impl<T: ?Sized> Zeroable for *const T {}

macro_rules! impl_tuple_zeroable {
    ($(,)?) => {};
    ($first:ident, $($t:ident),* $(,)?) => {
        // all elements are zeroable and padding can be zero
        unsafe impl<$first: Zeroable, $($t: Zeroable),*> Zeroable for ($first, $($t),*) {}
        impl_tuple_zeroable!($($t),* ,);
    }
}

impl_tuple_zeroable!(A, B, C, D, E, F, G, H, I, J);
