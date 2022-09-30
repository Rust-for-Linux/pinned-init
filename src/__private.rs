//! Workaround for specialization
use super::*;

mod sealed {
    use super::*;
    pub trait Sealed {}

    impl Sealed for Direct {}
    impl Sealed for Closure {}
}

pub trait InitWay: sealed::Sealed {}

impl InitWay for Direct {}
impl InitWay for Closure {}

pub struct Direct;
pub struct Closure;

pub unsafe trait __PinInitImpl<T, E, W: InitWay> {
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E>;
}
pub unsafe trait __InitImpl<T, E, W: InitWay>: __PinInitImpl<T, E, W> {
    unsafe fn __init(self, slot: *mut T) -> Result<(), E>;
}

unsafe impl<T> __PinInitImpl<T, Never, Direct> for T {
    unsafe fn __pinned_init(self, place: *mut T) -> Result<(), Never> {
        // SAFETY: pointer valid as per function contract
        unsafe { place.write(self) };
        Ok(())
    }
}

unsafe impl<T> __InitImpl<T, Never, Direct> for T {
    unsafe fn __init(self, place: *mut T) -> Result<(), Never> {
        // SAFETY: pointer valid as per function contract
        unsafe { place.write(self) };
        Ok(())
    }
}

unsafe impl<I, T, E> __InitImpl<T, E, Closure> for I
where
    I: Init<T, E>,
{
    unsafe fn __init(self, slot: *mut T) -> Result<(), E> {
        Init::__init(self, slot)
    }
}

unsafe impl<I, T, E> __PinInitImpl<T, E, Closure> for I
where
    I: PinInit<T, E>,
{
    unsafe fn __pinned_init(self, slot: *mut T) -> Result<(), E> {
        PinInit::__pinned_init(self, slot)
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

/// Stack initializer helper type. See [`stack_init`].
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

    /// # Safety
    /// The caller ensures that `self` is on the stack and not accesible to **any** other code.
    pub unsafe fn init<E>(&mut self, init: impl PinInit<T, E>) -> Result<Pin<&mut T>, E> {
        unsafe { init.__pinned_init(self.0.as_mut_ptr()) }?;
        self.1 = true;
        Ok(unsafe { Pin::new_unchecked(self.0.assume_init_mut()) })
    }
}
