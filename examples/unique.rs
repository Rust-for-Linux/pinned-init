#![deny(unsafe_op_in_unsafe_fn)]
#![feature(get_mut_unchecked, generic_associated_types)]
use core::{mem, ops::*, pin::*};
use pinned_init::{transmute::TransmuteInto, ptr::OwnedUniquePtr};
use std::sync::*;

#[repr(transparent)]
pub struct UniqueArc<T: ?Sized>(Arc<T>);

impl<T: ?Sized> UniqueArc<T> {
    pub fn new(t: T) -> Self
    where
        T: Sized,
    {
        Self(Arc::new(t))
    }

    pub fn share(self) -> Arc<T> {
        self.0
    }

    pub fn pin(t: T) -> Pin<Self>
    where
        T: Sized,
    {
        unsafe {
            // SAFETY: we have the only reference to t and we never move t.
            Pin::new_unchecked(Self::new(t))
        }
    }
}

impl<T: ?Sized> Deref for UniqueArc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T: ?Sized> DerefMut for UniqueArc<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            // SAFETY: we hold the only reference to this value.
            debug_assert_eq!(Arc::strong_count(&self.0) + Arc::weak_count(&self.0), 1);
            Arc::get_mut_unchecked(&mut self.0)
        }
    }
}

unsafe impl<T, U> TransmuteInto<UniqueArc<T>> for UniqueArc<U>
where
    U: TransmuteInto<T>,
{
    unsafe fn transmute_ptr(this: *const Self) -> *const UniqueArc<T> {
        unsafe { mem::transmute(this) }
    }
}

unsafe impl<T: ?Sized> OwnedUniquePtr<T> for UniqueArc<T> {
    type Ptr<U: ?Sized> = UniqueArc<U>;
}

fn main() {}
