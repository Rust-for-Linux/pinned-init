#![cfg_attr(not(feature = "std"), no_std)]
//
#![feature(never_type)]
#![feature(raw_ref_op)]
#![feature(unwrap_infallible)]
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

#[macro_export]
macro_rules! stack_init {
    ($var:ident = $val:expr) => {
        let mut $var = $crate::StackInit::uninit();
        let $var = $crate::StackInit::init(&mut $var, $val);
    };
}

/// Construct an in-place initializer for structs.
///
/// The syntax is identical to a normal struct initializer:
/// ```rust
/// # #![feature(never_type)]
/// # use simple_safe_init::*;
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
/// let initializer = pin_init!(Foo {
///     a,
///     b: Bar {
///         x: 64,
///     },
/// });
/// # let _: Result<Pin<Box<Foo>>, AllocInitErr<!>> = Box::pin_init(initializer);
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
/// # use simple_safe_init::*;
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
///     pub fn new() -> impl Initializer<Self, !> {
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
/// # use simple_safe_init::*;
/// # use core::pin::Pin;
/// # struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }
/// # struct Bar {
/// #     x: u32,
/// # }
/// # impl Foo {
/// #     pub fn new() -> impl Initializer<Self, !> {
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
/// They can also easily embedd it into their `struct`s:
/// ```rust
/// # #![feature(never_type)]
/// # use simple_safe_init::*;
/// # use core::pin::Pin;
/// # struct Foo {
/// #     a: usize,
/// #     b: Bar,
/// # }
/// # struct Bar {
/// #     x: u32,
/// # }
/// # impl Foo {
/// #     pub fn new() -> impl Initializer<Self, !> {
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
///     pub fn new(other: u32) -> impl Initializer<Self, !> {
///         init!(Self {
///             foo1: Foo::new(),
///             foo2: Foo::new(),
///             other,
///         })
///     }
/// }
/// ```
#[macro_export]
macro_rules! pin_init {
    ($(&$this:ident <- )? $t:ident $(<$($generics:ty),* $(,)?>)? {
        $($field:ident $(: $val:expr)?),*
        $(,)?
    }) => {{
        let init = move |place: *mut $t $(<$($generics),*>)?| -> ::core::result::Result<(), _> {
            $(let $this = unsafe { ::core::ptr::NonNull::new_unchecked(place) };)?
            $(
                $(let $field = $val;)?
                // call the initializer
                // SAFETY: place is valid, because we are inside of an initializer closure, we return
                //         when an error/panic occurs.
                unsafe { $crate::PinInitializer::__init_pinned($field, ::core::ptr::addr_of_mut!((*place).$field))? };
                // create the drop guard
                // SAFETY: we forget the guard later when initialization has succeeded.
                let $field = unsafe { $crate::DropGuard::new(::core::ptr::addr_of_mut!((*place).$field)) };
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
            Ok(())
        };
        let init = unsafe { $crate::PinInit::from_closure(init) };
        init
    }}
}

#[macro_export]
macro_rules! init {
    ($(where $this:ident <- )?$t:ident $(<$($generics:ty),* $(,)?>)? {
        $($field:ident $(: $val:expr)?),*
        $(,)?
    }) => {{
        let init = move |place: *mut $t $(<$($generics),*>)?| -> ::core::result::Result<(), _> {
            $(let $this = unsafe { ::core::ptr::NonNull::new_unchecked(place) };)?
            $(
                $(let $field = $val;)?
                // call the initializer
                // SAFETY: place is valid, because we are inside of an initializer closure, we return
                //         when an error/panic occurs.
                unsafe { $crate::Initializer::__init($field, ::core::ptr::addr_of_mut!((*place).$field))? };
                // create the drop guard
                // SAFETY: we forget the guard later when initialization has succeeded.
                let $field = unsafe { $crate::DropGuard::new(::core::ptr::addr_of_mut!((*place).$field)) };
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
            Ok(())
        };
        let init = unsafe { $crate::Init::from_closure(init) };
        init
    }}
}

mod sealed {
    use super::*;
    pub trait Sealed {}

    impl Sealed for Direct {}
    impl Sealed for Closure {}
}

/// Marking ways of initialization, there exist two:
/// - [`Direct`],
/// - [`Closure`].
///
/// This is necessary, because otherwise the implementations would overlap.
pub trait InitWay: sealed::Sealed {}

impl InitWay for Direct {}
impl InitWay for Closure {}

/// Direct value based initialization.
pub struct Direct;
/// Initialization via closure that initializes each field.
pub struct Closure;

/// An initializer for `T`.
///
/// # Safety
/// The [`PinInitializer::__init_pinned`] function
/// - returns `Ok(())` iff it initialized every field of place,
/// - returns `Err(err)` iff it encountered an error and then cleaned place, this means:
///     - place can be deallocated without UB ocurring,
///     - place does not need to be dropped,
///     - place is not partially initialized.
pub unsafe trait PinInitializer<T, E, Way: InitWay = Closure>: Sized {
    /// Initializes `place`.
    ///
    /// # Safety
    /// `place` is a valid pointer to uninitialized memory.
    /// The caller does not touch `place` when `Err` is returned, they are only permitted to
    /// deallocate.
    /// The place will not move, i.e. it will be pinned.
    unsafe fn __init_pinned(self, place: *mut T) -> Result<(), E>;
}

/// An initializer for `T`.
///
/// # Safety
/// The [`Initializer::__init`] function
/// - returns `Ok(())` iff it initialized every field of place,
/// - returns `Err(err)` iff it encountered an error and then cleaned place, this means:
///     - place can be deallocated without UB ocurring,
///     - place does not need to be dropped,
///     - place is not partially initialized.
///
/// Contrary to its supertype [`PinInitializer<T, E, Way>`] the caller is allowed to
/// move the pointee after initialization.
pub unsafe trait Initializer<T, E, Way: InitWay = Closure>:
    PinInitializer<T, E, Way>
{
    /// Initializes `place`.
    ///
    /// # Safety
    /// `place` is a valid pointer to uninitialized memory.
    /// The caller does not touch `place` when `Err` is returned, they are only permitted to
    /// deallocate.
    unsafe fn __init(self, place: *mut T) -> Result<(), E>;
}

unsafe impl<T> PinInitializer<T, !, Direct> for T {
    unsafe fn __init_pinned(self, place: *mut T) -> Result<(), !> {
        unsafe {
            place.write(self);
        }
        Ok(())
    }
}

unsafe impl<T> Initializer<T, !, Direct> for T {
    unsafe fn __init(self, place: *mut T) -> Result<(), !> {
        unsafe {
            place.write(self);
        }
        Ok(())
    }
}

type Invariant<T> = PhantomData<fn(T) -> T>;

/// A closure initializer.
pub struct Init<F, T, E>(F, Invariant<(T, E)>);

impl<T, E, F> Init<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    /// Creates a new Init from the given closure
    ///
    /// # Safety
    /// The closure
    /// - returns `Ok(())` iff it initialized every field of place,
    /// - returns `Err(err)` iff it encountered an error and then cleaned place, this means:
    ///     - place can be deallocated without UB ocurring,
    ///     - place does not need to be dropped,
    ///     - place is not partially initialized.
    /// - place may move after initialization
    pub const unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

unsafe impl<T, F, E> PinInitializer<T, E, Closure> for Init<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn __init_pinned(self, place: *mut T) -> Result<(), E> {
        (self.0)(place)
    }
}

unsafe impl<T, F, E> Initializer<T, E, Closure> for Init<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn __init(self, place: *mut T) -> Result<(), E> {
        (self.0)(place)
    }
}

/// A closure initializer for pinned data.
pub struct PinInit<F, T, E>(F, Invariant<(T, E)>);

impl<T, E, F> PinInit<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    /// Creates a new Init from the given closure
    ///
    /// # Safety
    /// The closure
    /// - returns `Ok(())` iff it initialized every field of place,
    /// - returns `Err(err)` iff it encountered an error and then cleaned place, this means:
    ///     - place can be deallocated without UB ocurring,
    ///     - place does not need to be dropped,
    ///     - place is not partially initialized.
    pub const unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

unsafe impl<T, F, E> PinInitializer<T, E> for PinInit<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn __init_pinned(self, place: *mut T) -> Result<(), E> {
        (self.0)(place)
    }
}

/// When a value of this type is dropped, it drops something else.
pub struct DropGuard<T: ?Sized>(*mut T);

impl<T: ?Sized> DropGuard<T> {
    /// Creates a new [`DropGuard<T>`]. It will [`ptr::drop_in_place`] `ptr` when it gets dropped.
    ///
    /// # Safety
    /// `ptr` must be a valid poiner.
    ///
    /// It is the callers responsibility that `self` will only get dropped if the pointee of `ptr`:
    /// - has not been dropped,
    /// - is not accesible by any other means,
    /// - will not be dropped by any other means.
    pub unsafe fn new(ptr: *mut T) -> Self {
        Self(ptr)
    }
}

impl<T: ?Sized> Drop for DropGuard<T> {
    fn drop(&mut self) {
        // SAFETY: safe as a `DropGuard` can only be constructed using the unsafe new function.
        unsafe { ptr::drop_in_place(self.0) }
    }
}

pub struct StackInit<T>(MaybeUninit<T>, bool);

impl<T> Drop for StackInit<T> {
    fn drop(&mut self) {
        if self.1 {
            unsafe { self.0.assume_init_drop() };
        }
    }
}
impl<T> StackInit<T> {
    pub fn uninit() -> Self {
        Self(MaybeUninit::uninit(), false)
    }

    pub fn init<Way: InitWay>(&mut self, init: impl PinInitializer<T, !, Way>) -> Pin<&mut T> {
        unsafe { init.__init_pinned(self.0.as_mut_ptr()).into_ok() };
        self.1 = true;
        unsafe { Pin::new_unchecked(self.0.assume_init_mut()) }
    }
}

pub trait InPlaceInit<T>: Sized {
    type Error<E>;

    fn pin_init<E, Way: InitWay>(
        init: impl PinInitializer<T, E, Way>,
    ) -> Result<Pin<Self>, Self::Error<E>>;

    fn init<E, Way: InitWay>(init: impl Initializer<T, E, Way>) -> Result<Self, Self::Error<E>>;
}

#[derive(Debug)]
pub enum AllocInitErr<E> {
    Init(E),
    Alloc,
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<E> From<AllocError> for AllocInitErr<E> {
    fn from(_: AllocError) -> Self {
        Self::Alloc
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<T> InPlaceInit<T> for Box<T> {
    type Error<E> = AllocInitErr<E>;

    fn pin_init<E, Way: InitWay>(
        init: impl PinInitializer<T, E, Way>,
    ) -> Result<Pin<Self>, Self::Error<E>> {
        let mut this = Box::try_new_uninit()?;
        let place = this.as_mut_ptr();
        // SAFETY: when init errors/panics, place will get deallocated but not dropped,
        // place is valid and will not be moved because of the into_pin
        unsafe { init.__init_pinned(place).map_err(AllocInitErr::Init)? };
        // SAFETY: all fields have been initialized
        Ok(Box::into_pin(unsafe { this.assume_init() }))
    }

    fn init<E, Way: InitWay>(init: impl Initializer<T, E, Way>) -> Result<Self, Self::Error<E>> {
        let mut this = Box::try_new_uninit()?;
        let place = this.as_mut_ptr();
        // SAFETY: when init errors/panics, place will get deallocated but not dropped,
        // place is valid
        unsafe { init.__init(place).map_err(AllocInitErr::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { this.assume_init() })
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<T> InPlaceInit<T> for Arc<T> {
    type Error<E> = AllocInitErr<E>;

    fn pin_init<E, Way: InitWay>(
        init: impl PinInitializer<T, E, Way>,
    ) -> Result<Pin<Self>, Self::Error<E>> {
        let mut this = Arc::try_new_uninit()?;
        let place = unsafe { Arc::get_mut_unchecked(&mut this) }.as_mut_ptr();
        // SAFETY: when init errors/panics, place will get deallocated but not dropped,
        // place is valid and will not be moved because of the into_pin
        unsafe { init.__init_pinned(place).map_err(AllocInitErr::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { Pin::new_unchecked(this.assume_init()) })
    }

    fn init<E, Way: InitWay>(init: impl Initializer<T, E, Way>) -> Result<Self, Self::Error<E>> {
        let mut this = Arc::try_new_uninit()?;
        let place = unsafe { Arc::get_mut_unchecked(&mut this) }.as_mut_ptr();
        // SAFETY: when init errors/panics, place will get deallocated but not dropped,
        // place is valid
        unsafe { init.__init(place).map_err(AllocInitErr::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { this.assume_init() })
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<T> InPlaceInit<T> for Rc<T> {
    type Error<E> = AllocInitErr<E>;

    fn pin_init<E, Way: InitWay>(
        init: impl PinInitializer<T, E, Way>,
    ) -> Result<Pin<Self>, Self::Error<E>> {
        let mut this = Rc::try_new_uninit()?;
        let place = unsafe { Rc::get_mut_unchecked(&mut this) }.as_mut_ptr();
        // SAFETY: when init errors/panics, place will get deallocated but not dropped,
        // place is valid and will not be moved because of the into_pin
        unsafe { init.__init_pinned(place).map_err(AllocInitErr::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { Pin::new_unchecked(this.assume_init()) })
    }

    fn init<E, Way: InitWay>(init: impl Initializer<T, E, Way>) -> Result<Self, Self::Error<E>> {
        let mut this = Rc::try_new_uninit()?;
        let place = unsafe { Rc::get_mut_unchecked(&mut this) }.as_mut_ptr();
        // SAFETY: when init errors/panics, place will get deallocated but not dropped,
        // place is valid
        unsafe { init.__init(place).map_err(AllocInitErr::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { this.assume_init() })
    }
}
