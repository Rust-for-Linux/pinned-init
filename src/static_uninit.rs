//! Using [`MaybeUninit<T>`] requires unsafe, but this is often not necessary,
//! because the type system can statically determine the initialization status.
//!
//! This module provides [`StaticUninit<T, INIT>`] a safe alternative using
//! static type checking to ensure one cannot use an uninitialized value as an
//! initialized and to prevent leaking values when initializing a value twice
//! without dropping the contents.

use core::{
    borrow::{Borrow, BorrowMut},
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    ptr::addr_of_mut,
};

/// This type is similar to [`MaybeUninit`], but it provides a safe interface
/// for initialization which can then be used statically.
/// It represents a Value which is
/// - uninitialized iff INIT == false
/// - initialized iff INIT == true
///
/// `StaticUninit<T, true>` behaves like `T`, as it implements [`DerefMut`] and
/// allows taking the value directly [`StaticUninit::into_inner`].
///
/// It also gives access to an unsafe interface allowing arbitrary modifications
/// of the underlying [`MaybeUninit`] if there are more complex initialization
/// requirements.
///
/// # Safety
///
/// If at any point you use one of the unsafe methods to access and modify the
/// inner [`MaybeUninit<T>`], you need to keep track of the initialization state
/// of that particular [`StaticUninit`] until you pass it to some other part of
/// the code. It is unsound to release a [`StaticUninit`] in
/// - a partially initialized state
/// - an unknown state of initialization
/// to other code, [`StaticUninit`] always is either fully initialized, or fully
/// uninitialized.
/// To achive partial initialization, use smaller components that can be fully
/// initialized seperatly and then create a wrapper struct using multiple
/// wrapping [`StaticUninit`]s (this is achieved by the `#[pinned_init]` proc
/// macro attribute).
#[repr(transparent)]
pub struct StaticUninit<T, const INIT: bool> {
    inner: MaybeUninit<T>,
}

impl<T, const INIT: bool> Drop for StaticUninit<T, INIT> {
    fn drop(&mut self) {
        if INIT {
            unsafe {
                // SAFETY: we are statically known to be initialized, so drop our value
                self.inner.assume_init_drop();
            }
        }
    }
}

impl<T> Deref for StaticUninit<T, true> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe {
            // SAFETY: we are statically known to be initialized.
            self.inner.assume_init_ref()
        }
    }
}

impl<T> DerefMut for StaticUninit<T, true> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            // SAFETY: we are statically known to be initialized.
            self.inner.assume_init_mut()
        }
    }
}

impl<T> Borrow<T> for StaticUninit<T, true> {
    #[inline]
    fn borrow(&self) -> &T {
        &*self
    }
}

impl<T> BorrowMut<T> for StaticUninit<T, true> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        &mut *self
    }
}

impl<T> AsRef<T> for StaticUninit<T, true> {
    #[inline]
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T> AsMut<T> for StaticUninit<T, true> {
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        &mut *self
    }
}

impl<T> From<T> for StaticUninit<T, true> {
    #[inline]
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<T> StaticUninit<T, true> {
    /// Creates an already initialized `T` with its init status statically
    /// tracked.
    #[inline]
    pub fn new(data: T) -> Self {
        Self {
            inner: MaybeUninit::new(data),
        }
    }

    /// Retrieve the inner value of this `StaticUninit`.
    #[inline]
    pub fn into_inner(self) -> T {
        unsafe {
            // SAFETY: we are statically known to be initialized.
            self.inner.assume_init_read()
        }
    }

    /// Gets a mutable pointer to the initialized value. This avoids creating a reference, allowing
    /// mutable aliasing using `*mut`. This function is inspired by
    /// [raw_get](https://doc.rust-lang.org/std/cell/struct.UnsafeCell.html#method.raw_get) from UnsafeCell.
    ///
    /// # Safety
    ///
    /// The supplied pointer must be valid.
    ///
    /// When casting the returned pointer to
    /// - `&mut T` the caller needs to ensure that no other references exist.
    /// - `&T` the caller needs to ensure that no mutable references exist.
    pub unsafe fn raw_get(this: *mut Self) -> *mut T {
        unsafe {
            // SAFETY: this is a valid pointer and we are initialized.
            // `MaybeUninit` is `repr(transparent)`, so we can cast the pointer to `T`.
            addr_of_mut!((*this).inner) as *mut T
        }
    }
}

impl<T> StaticUninit<T, false> {
    /// Creates a new uninitialized `T` with its init status statically tracked.
    #[inline]
    pub fn uninit() -> Self {
        Self {
            inner: MaybeUninit::uninit(),
        }
    }

    /// Gives access to the inner [`MaybeUninit`] immutably.
    ///
    /// # Safety
    ///
    /// You need to keep track of the initialization state of `self`, it is
    /// unsound to leave a [`StaticUninit`] in
    /// - a partially initialized state
    /// - an unknown state of initialization
    /// and allow code to observe it unknowingly.
    #[inline]
    pub unsafe fn as_uninit_ref(&self) -> &MaybeUninit<T> {
        &self.inner
    }

    /// Gives access to the inner [`MaybeUninit`] mutably.
    ///
    /// # Safety
    ///
    /// You need to keep track of the initialization state of `self`, it is
    /// unsound to leave a [`StaticUninit`] in
    /// - a partially initialized state
    /// - an unknown state of initialization
    /// and allow code to observe it unknowingly.
    #[inline]
    pub unsafe fn as_uninit_mut(&mut self) -> &mut MaybeUninit<T> {
        &mut self.inner
    }

    /// Initializes `self` using `data` and tracks that status statically.
    #[inline]
    pub fn write(self, data: T) -> StaticUninit<T, true> {
        StaticUninit::new(data)
    }
}
