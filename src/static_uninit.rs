//! Using [`MaybeUninit<T>`] requires unsafe, but this is often not necessary.
//! This module provides [`StaticUninit<T, INIT>`] a safer alternative using
//! static type checking to ensure one cannot use an uninitialized value as an
//! initialized and to prevent leaking values when initializing a value twice
//! without dropping the contents.

use core::{mem::*, ops::*};

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
            self.inner.assume_init()
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
