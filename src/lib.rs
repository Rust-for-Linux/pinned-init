#![feature(
    type_alias_impl_trait,
    generic_associated_types,
    never_type,
    stmt_expr_attributes,
    raw_ref_op,
    proc_macro_hygiene,
    new_uninit
)]
use core::{marker::PhantomData, mem::MaybeUninit, pin::Pin};

pub use safe_init_internal::*;

pub trait Place: Sized {
    unsafe fn init<F, E, I: Initializer<Self, E, F>>(place: *mut Self, init: I) -> Result<(), E>;
}

pub trait Initializer<T, E, F = ()> {
    unsafe fn init(self, place: *mut T) -> Result<(), E>;
}

impl<T> Place for T {
    unsafe fn init<F, E, I: Initializer<Self, E, F>>(place: *mut T, init: I) -> Result<(), E> {
        unsafe { init.init(place) }
    }
}

impl<T> Initializer<T, !, !> for T {
    unsafe fn init(self, place: *mut T) -> Result<(), !> {
        unsafe {
            place.write(self);
        }
        Ok(())
    }
}

pub struct Init<F, T, E>(F, PhantomData<fn(T, E) -> (T, E)>);

impl<T, E, F> Init<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    pub unsafe fn from_closure(f: F) -> Self {
        Self(f, PhantomData)
    }
}

impl<T, F, E> Initializer<T, E> for Init<F, T, E>
where
    F: FnOnce(*mut T) -> Result<(), E>,
{
    unsafe fn init(self, place: *mut T) -> Result<(), E> {
        (self.0)(place)
    }
}

pub trait BoxExt<T>: Sized {
    fn pin_init<E, F>(init: impl Initializer<T, E, F>) -> Result<Pin<Self>, E>;
}

impl<T> BoxExt<T> for Box<T> {
    fn pin_init<E, F>(init: impl Initializer<T, E, F>) -> Result<Pin<Self>, E> {
        let mut this = Box::new(MaybeUninit::uninit());
        let place = this.as_mut_ptr();
        unsafe { init.init(place)? };
        Ok(Box::into_pin(unsafe { this.assume_init() }))
    }
}
