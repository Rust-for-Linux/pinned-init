#![cfg_attr(not(feature = "std"), no_std)]
#![feature(generic_associated_types)]
#![feature(never_type)]
#![feature(raw_ref_op)]
#![feature(allocator_api)]
#![cfg_attr(any(feature = "alloc", feature = "std"), feature(new_uninit))]
#![cfg_attr(feature = "attr", feature(proc_macro_hygiene))]
#![cfg_attr(feature = "attr", feature(stmt_expr_attributes))]
#[cfg(feature = "alloc")]
use alloc::alloc::AllocError;
use core::{marker::PhantomData, pin::Pin, ptr};
#[cfg(feature = "std")]
use std::alloc::AllocError;

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;

pub use simple_safe_init_internal::{init, pin_init};

#[cfg(feature = "attr")]
pub mod attr {
    pub use simple_safe_init_internal::{init_attr as init, pin_init_attr as pin_init};
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
/// The [`PinInitializer::init`] function
/// - returns `Ok(())` iff it initialized every field of place,
/// - returns `Err(err)` iff it encountered an error and then cleaned place, this means:
///     - place can be deallocated without UB ocurring,
///     - place does not need to be dropped,
///     - place is not partially initialized.
pub unsafe trait PinInitializer<T, E, Way: InitWay = Closure> {
    /// Initializes `place`.
    ///
    /// # Safety
    /// `place` is a valid pointer to uninitialized memory.
    /// The caller does not touch `place` when `Err` is returned, they are only permitted to
    /// deallocate.
    /// The place will not move, i.e. it will be pinned.
    unsafe fn init(self, place: *mut T) -> Result<(), E>;
}

/// An initializer for `T`.
///
/// # Safety
/// The [`PinInitializer::init`] function
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
}

unsafe impl<T> PinInitializer<T, !, Direct> for T {
    unsafe fn init(self, place: *mut T) -> Result<(), !> {
        unsafe {
            place.write(self);
        }
        Ok(())
    }
}

unsafe impl<T> Initializer<T, !, Direct> for T {}

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
    pub unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

unsafe impl<T, F, E> PinInitializer<T, E, Closure> for Init<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn init(self, place: *mut T) -> Result<(), E> {
        (self.0)(place)
    }
}

unsafe impl<T, F, E> Initializer<T, E, Closure> for Init<F, T, E> where
    F: FnOnce(*mut T) -> Result<(), E>
{
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
    pub unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

unsafe impl<T, F, E> PinInitializer<T, E> for PinInit<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn init(self, place: *mut T) -> Result<(), E> {
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

#[cfg(any(feature = "alloc", feature = "std"))]
#[derive(Debug)]
pub enum BoxInitErr<E> {
    Init(E),
    Alloc,
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<E> From<AllocError> for BoxInitErr<E> {
    fn from(_: AllocError) -> Self {
        Self::Alloc
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
pub trait BoxExt<T>: Sized {
    type Error<E>;

    fn pin_init<E, Way: InitWay>(
        init: impl PinInitializer<T, E, Way>,
    ) -> Result<Pin<Self>, Self::Error<E>>;
    fn init<E, Way: InitWay>(init: impl Initializer<T, E, Way>) -> Result<Self, Self::Error<E>>;
}

#[cfg(any(feature = "alloc", feature = "std"))]
impl<T> BoxExt<T> for Box<T> {
    type Error<E> = BoxInitErr<E>;

    fn pin_init<E, Way: InitWay>(
        init: impl PinInitializer<T, E, Way>,
    ) -> Result<Pin<Self>, Self::Error<E>> {
        let mut this = Box::try_new_uninit()?;
        let place = this.as_mut_ptr();
        // SAFETY: when init errors/panics, place will get deallocated but not dropped,
        // place is valid and will not be moved because of the into_pin
        unsafe { init.init(place).map_err(BoxInitErr::Init)? };
        // SAFETY: all fields have been initialized
        Ok(Box::into_pin(unsafe { this.assume_init() }))
    }

    fn init<E, Way: InitWay>(init: impl Initializer<T, E, Way>) -> Result<Self, Self::Error<E>> {
        let mut this = Box::try_new_uninit()?;
        let place = this.as_mut_ptr();
        // SAFETY: when init errors/panics, place will get deallocated but not dropped,
        // place is valid
        unsafe { init.init(place).map_err(BoxInitErr::Init)? };
        // SAFETY: all fields have been initialized
        Ok(unsafe { this.assume_init() })
    }
}
