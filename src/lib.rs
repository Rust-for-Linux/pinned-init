#![feature(
    type_alias_impl_trait,
    generic_associated_types,
    never_type,
    stmt_expr_attributes,
    raw_ref_op,
    proc_macro_hygiene,
    new_uninit
)]
use core::{marker::PhantomData, mem::MaybeUninit, pin::Pin, ptr};

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

pub trait InitWay: sealed::Sealed {}

impl InitWay for Direct {}
impl InitWay for Closure {}

pub struct Direct;
pub struct Closure;

pub trait PinInitializer<T, E, Way: InitWay = Closure> {
    unsafe fn init(self, place: *mut T) -> Result<(), E>;
}

pub trait Initializer<T, E, Way: InitWay = Closure>: PinInitializer<T, E, Way> {}

impl<T> PinInitializer<T, !, Direct> for T {
    unsafe fn init(self, place: *mut T) -> Result<(), !> {
        unsafe {
            place.write(self);
        }
        Ok(())
    }
}

impl<T> Initializer<T, !, Direct> for T {}

pub struct Init<F, T, E>(F, PhantomData<fn(T, E) -> (T, E)>);

impl<T, E, F> Init<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    pub unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

impl<T, F, E> PinInitializer<T, E, Closure> for Init<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn init(self, place: *mut T) -> Result<(), E> {
        (self.0)(place)
    }
}

impl<T, F, E> Initializer<T, E, Closure> for Init<F, T, E> where F: FnOnce(*mut T) -> Result<(), E> {}

pub struct PinInit<F, T, E>(F, PhantomData<fn(T, E) -> (T, E)>);

impl<T, E, F> PinInit<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    pub unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

impl<T, F, E> PinInitializer<T, E> for PinInit<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn init(self, place: *mut T) -> Result<(), E> {
        (self.0)(place)
    }
}

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
        unsafe { ptr::drop_in_place(self.0) }
    }
}

pub trait BoxExt<T>: Sized {
    fn pin_init<E, Way: InitWay>(init: impl PinInitializer<T, E, Way>) -> Result<Pin<Self>, E>;
}

impl<T> BoxExt<T> for Box<T> {
    fn pin_init<E, Way: InitWay>(init: impl PinInitializer<T, E, Way>) -> Result<Pin<Self>, E> {
        let mut this = Box::new(MaybeUninit::uninit());
        let place = this.as_mut_ptr();
        unsafe { init.init(place)? };
        Ok(Box::into_pin(unsafe { this.assume_init() }))
    }
}
