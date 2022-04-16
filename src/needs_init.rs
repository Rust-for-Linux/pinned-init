//! Custom pointer types used to ensure that initialization was done completly.
//! The pointer types provided by this module [`NeedsPinnedInit<'init, T>`] and
//! [`NeedsInit<'init, T>`] will
//! - panic when debug assertions are turned on
//! - produce link time erros when debug assertions are turned off
//! when they determine, that a value has not been initialized.
//!
//! This catches simple errors when forgetting a variable.

use crate::{private::BeginInit, static_uninit::StaticUninit, PinnedInit};
use core::{mem, pin::Pin};

/// A pointer to pinned data that needs to be initialized while pinned.
/// When this pointer is neglected and not initialized, it will
/// - panic on drop (when debug assertions are enabled)
/// - produce a link time error (when debug assertions are disabled)
/// This is to prevent partial initialization and guarantee statically (when
/// used without debug assertions) that the type `T` is fully initialized and
/// may be transmuted to its initialized form.
#[repr(transparent)]
pub struct NeedsPinnedInit<'init, T: ?Sized> {
    inner: Option<Pin<&'init mut T>>,
}

#[cfg(feature = "assert_init")]
impl<'init, T: ?Sized> Drop for NeedsPinnedInit<'init, T> {
    fn drop(&mut self) {
        if_cfg! {
            if (debug_assertions) {
                panic!(
                    "NeedsPinnedInit({:p}) was dropped, without prior initialization!",
                    *self.inner.as_ref().unwrap()
                );
            } else {
                extern "C" {
                    #[link_name = "NeedsPinnedInit was dropped, without prior initialization."]
                    fn trigger() -> !;
                }
                unsafe {
                    trigger();
                }
            }
        }
    }
}

impl<'init, T: ?Sized> NeedsPinnedInit<'init, T> {
    /// Construct a new `NeedsPinnedInit` from the given [`Pin`].
    ///
    /// # Safety
    ///
    /// When the `'init` lifetime expires, the value at `inner` has been
    /// initialized and thus changed type from `T` to `T::Initialized`.
    /// The caller needs to guarantee that no pointers to `inner` exist that are
    /// unaware of the call to this function. An aware pointer will change its
    /// pointee type to `T::Initialized` when `'init` expires.
    #[inline]
    pub unsafe fn new_unchecked(inner: Pin<&'init mut T>) -> Self
    where
        T: PinnedInit,
    {
        Self { inner: Some(inner) }
    }

    /// Begin to initialize the value behind this `NeedsPinnedInit`.
    #[inline]
    pub fn begin_init(mut self) -> <T as BeginInit>::OngoingInit<'init>
    where
        T: PinnedInit,
    {
        let res = if let Some(inner) = self.inner.take() {
            unsafe { inner.__begin_init() }
        } else {
            if_cfg! {
                if (feature = "mark-unreachable") {
                    unsafe {
                        core::hint::unreachable_unchecked();
                    }
                } else {
                    unreachable!();
                }
            }
        };
        mem::forget(self);
        res
    }

    /// Get a raw const pointer to the value behind this `NeedsPinnedInit`.
    ///
    /// # Safety
    ///
    /// The caller needs to carefully handle this pointer, because
    /// - it is only valid for the duration that the underlying value T will live
    /// for (also counting the lifetime of the value when it is `T::Initialized`).
    /// - it is pinned.
    /// - you may only generate a `*mut T` or `&mut T` if the value of `T` is
    /// wrapped in an [`UnsafeCell`].
    /// - this pointer needs to be aware, that the type of the value pointed to
    /// will change from `T` to `T::Initialized` when `'init` expires.
    ///
    /// Storing this pointer inside of the `T` itself for example is sound.
    #[inline]
    pub unsafe fn as_ptr(&self) -> *const T {
        (&*self.inner.as_ref().unwrap().as_ref()) as *const T
    }

    /// Get a raw mutable pointer to the value behind this `NeedsPinnedInit`.
    ///
    /// # Safety
    ///
    /// The caller needs to carefully handle this pointer, because
    /// - it is only valid for the duration that the underlying value T will live
    /// for (also counting the lifetime of the value when it is `T::Initialized`).
    /// - it is pinned.
    /// - you may only call this function if the value of `T` is wrapped in an
    /// [`UnsafeCell`].
    /// - this pointer needs to be aware, that the type of the value pointed to
    /// will change from `T` to `T::Initialized` when `'init` expires.
    ///
    /// Storing this pointer inside of the `T` itself for example is sound.
    #[inline]
    pub unsafe fn as_ptr_mut(&mut self) -> *mut T {
        unsafe { self.inner.as_mut().unwrap().as_mut().get_unchecked_mut() as *mut T }
    }
}
/// A pointer to data that needs to be initialized.
/// When this pointer is neglected and not initialized, it will
/// - panic on drop (when debug assertions are enabled)
/// - produce a link time error (when debug assertions are disabled)
/// This is to prevent partial initialization and guarantee statically (when
/// used without debug assertions) that the type `T` is fully initialized and
/// may be transmuted to its initialized form.
#[repr(transparent)]
pub struct NeedsInit<'init, T: ?Sized> {
    inner: &'init mut T,
}

#[cfg(feature = "assert_init")]
impl<'init, T: ?Sized> Drop for NeedsInit<'init, T> {
    fn drop(&mut self) {
        if_cfg! {
            if (debug_assertions) {
                panic!(
                    "NeedsInit({:p}) was dropped, without prior initialization!",
                    self.inner
                );
            } else {
                extern "C" {
                    #[link_name = "NeedsInit was dropped, without prior initialization."]
                    fn trigger() -> !;
                }
                unsafe {
                    trigger();
                }
            }
        }
    }
}

impl<'init, T: ?Sized> NeedsInit<'init, T> {
    /// Construct a new `NeedsInit` from the given pointer.
    ///
    /// # Safety
    ///
    /// When the `'init` lifetime expires, the value at `inner` has been
    /// initialized and thus changed type from `T` to `T::Initialized`.
    /// The caller needs to guarantee that no pointers to `inner` exist that are
    /// unaware of the call to this function. An aware pointer will change its
    /// pointee type to `T::Initialized` when `'init` expires.
    /// From the moment this function is called, this function may assume full
    /// control over the pointee until `'init` expires.
    #[inline]
    pub unsafe fn new_unchecked(inner: &'init mut T) -> Self {
        Self { inner }
    }
}

impl<'init, T> NeedsInit<'init, StaticUninit<T, false>> {
    /// Initializes the value behind this pointer with the supplied value
    pub fn init(self, value: T) {
        unsafe {
            // SAFETY: We have been constructed by [`NeedsInit::new_unchecked`]
            // and thus have full control over the pointee, we now change its
            // type and never access it again (we are consumed). When `'init`
            // expires and all accesses to the pointee are made through
            // [`StaticUninit<T, true>`] which is the initialized variant.
            //
            // This satisfies the contract of [`StaticUninit::as_uninit_mut`].
            self.inner.as_uninit_mut().write(value);
        }
        // We do not want to drop `self` here, because it would panic or link to
        // an unresolved symbol.
        mem::forget(self);
    }
}
