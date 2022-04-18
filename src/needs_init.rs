//! Custom pointer types used to ensure that initialization was done completly.
//! The pointer types provided by this module [`NeedsPinnedInit<'init, T>`] and
//! [`NeedsInit<'init, T>`] will
//! - panic when debug assertions are turned on
//! - produce link time erros when debug assertions are turned off
//! when they determine, that a value has not been initialized.
//!
//! This catches simple errors when forgetting a variable.
//!
//! The link time errors emitted by the linker will look overwhelming and are
//! not very helpful (you might be able to interpret the LLVM symbol where the
//! link error originated), instead they help you extend your tests or check your
//! unused variable warnings with more scrutiny.
//!
//! <details><summary><b>Example Linker error</b></summary>
//! ```text
//! TODO ERROR
//! ```
//! TODO
//! ```rust
//! TODO EXAMPLE CODE
//! ```
//! When running the debug build instead, we end up with the following panic,
//! which is a lot more helpful at providing the relevant information:
//! ```text
//! TODO PANIC
//! ```
//! </details>
//!
//! # Behaviour rationale
//!
//! The correctness of the link error relies on the compiler to optimize the
//! drop glue away. This is of course only possible, if optimizations are
//! enabled.
//! If you encounter such a link error and receive no runtime errors despite
//! running all initializer functions and being very sure that you did not
//! forget initializing a value, then feel free to report this issue at [my
//! repo](https://github.com/y86-dev/pinned-init/).
//!
//! If in some rare cases the compiler is unable to determine that all instances
//! of [`NeedsPinnedInit`] and [`NeedsInit`] are initialized, you are able to
//! disable the static and dynamic initialization check provided by this module:
//! pass the `pinned_init_unsafe_no_enforce_init` flag to rustc:
//! ```text
//! RUSTFLAGS="--cfg pinned_init_unsafe_no_enforce_init" cargo build
//! ```
//! This will also need to be done by all crates, that depend on your crate,
//! because it is circumventing one of the safety guarantees of this crate and
//! is expicitly opt-in. Please try to find a safe workaround or open an issue
//! at [my repo].

use crate::{private::BeginInit, static_uninit::StaticUninit, PinnedInit};
use core::{mem, pin::Pin};

/// A pointer to pinned data that needs to be initialized while pinned.
/// When this pointer is neglected and not initialized, it will
/// - panic on drop (when debug assertions are enabled)
/// - produce a link time error (when debug assertions are disabled)
/// This is to prevent partial initialization and guarantee statically (when
/// used without debug assertions) that the type `T` is fully initialized and
/// may be transmuted to its initialized form.
///
/// This pointer does **not** implement [`Deref`] or [`DerefMut`], instead you
/// should only use [`NeedsPinnedInit::begin_init`] on this type to begin safe
/// initialization of the inner `T`.
///
/// # Invariants and assumptions
///
/// - When the `'init` lifetime expires, the value this pointer pointed to, will
/// be initialized and have changed its type to `T::Initialized`.
/// - From the construction of a [`NeedsPinnedInit<'init, T>`] until the end of
/// `'init` it assumes full control over the pointee. This means that no one
/// else is allowed to access the underlying value.
///
/// [`Deref`]: core::ops::Deref
/// [`DerefMut`]: core::ops::DerefMut
#[repr(transparent)]
pub struct NeedsPinnedInit<'init, T: PinnedInit> {
    // need option here, otherwise `mem::forget`ing `self` in `begin_init` is
    // not possible, because we would need to borrow `self` for `'init`, but
    // `'init` ends only after we have been dropped.
    inner: Option<Pin<&'init mut T>>,
}

#[cfg(not(pinned_init_unsafe_no_enforce_init))]
impl<'init, T: PinnedInit> Drop for NeedsPinnedInit<'init, T> {
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
                    // SAFETY: this function does not exist and will generate a
                    // link error
                    trigger();
                }
            }
        }
    }
}

impl<'init, T: PinnedInit> NeedsPinnedInit<'init, T> {
    /// Construct a new `NeedsPinnedInit` from the given [`Pin`].
    ///
    /// # Safety
    ///
    /// - When the `'init` lifetime expires, the value at `inner` will be
    /// initialized and have changed type to `T::Initialized`.
    /// - From the moment this function is called until the end of `'init` the
    /// produced [`NeedsPinnedInit`] becomes the only valid way to access the
    /// underlying value.
    /// - The caller needs to guarantee, that the pointer from which `inner` was
    /// derived changes its pointee type to `T::Initialized`, when `'init` ends.
    #[inline]
    pub unsafe fn new_unchecked(inner: Pin<&'init mut T>) -> Self {
        Self { inner: Some(inner) }
    }

    /// Begin to initialize the value behind this `NeedsPinnedInit`.
    #[inline]
    pub fn begin_init(mut self) -> <T as BeginInit>::OngoingInit<'init> {
        let res = if let Some(inner) = self.inner.take() {
            unsafe {
                // SAFETY: API internal contract is upheld, __begin_init has the
                // same invariants as NeedsPinnedInit.
                inner.__begin_init()
            }
        } else {
            unsafe {
                // SAFETY: self.inner is never `take`n anywhere else. Because
                // this function takes `self` by value, we know, that the option
                // is populated, because we only create `NeedsPinnedInit` with
                // inner set to `Some`.
                core::hint::unreachable_unchecked();
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
    /// - dereferencing the pointer outside of the initialization process of `T`,
    /// while `'init` has not expired, is illegal (this would violate an invariant
    /// of [`NeedsPinnedInit`]).
    /// - this pointer needs to be aware, that the type of the value pointed to
    /// will change from `T` to `T::Initialized` when `'init` expires.
    ///
    /// Storing this pointer inside of the `T` itself for example is sound.
    ///
    /// [`UnsafeCell`]: core::cell::UnsafeCell
    #[inline]
    pub unsafe fn as_ptr(&self) -> *const T {
        let ptr: Pin<&T> = unsafe {
            // SAFETY: self.inner can only be None, if `begin_init` was called,
            // which takes `self` by value, so this function cannot be called.
            // `NeedsPinnedInit` is only initialized with Some, so inner is
            // populated.
            self.inner.as_ref().unwrap_unchecked()
        }
        .as_ref();
        (&*ptr) as *const T
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
    /// - dereferencing the pointer outside of the initialization process of `T`,
    /// while `'init` has not expired, is illegal (this would violate an invariant
    /// of [`NeedsPinnedInit`]).
    /// - this pointer needs to be aware, that the type of the value pointed to
    /// will change from `T` to `T::Initialized` when `'init` expires.
    ///
    /// Storing this pointer inside of the `T` itself for example is sound.
    ///
    /// [`UnsafeCell`]: core::cell::UnsafeCell
    #[inline]
    pub unsafe fn as_ptr_mut(&mut self) -> *mut T {
        let ptr: Pin<&mut T> = unsafe {
            // SAFETY: self.inner can only be None, if `begin_init` was called,
            // which takes `self` by value, so this function cannot be called.
            // `NeedsPinnedInit` is only initialized with Some, so inner is
            // populated.
            self.inner.as_mut().unwrap_unchecked()
        }
        .as_mut();
        unsafe {
            // SAFETY: the caller is responsible to handle this pointer as a
            // pinned pointer.
            ptr.get_unchecked_mut() as *mut T
        }
    }
}

/// A pointer to data that needs to be initialized.
/// When this pointer is neglected and not initialized, it will
/// - panic on drop (when debug assertions are enabled)
/// - produce a link time error (when debug assertions are disabled)
/// This is to prevent partial initialization and guarantee statically (when
/// used without debug assertions) that the type `T` is fully initialized.
///
/// This pointer does **not** implement [`Deref`] or [`DerefMut`], instead you
/// should only use [`NeedsInit::init`] on this type to safely initialize
/// the inner `T`.
///
/// # Invariants and assumptions
///
/// - When the `'init` lifetime expires, the value this pointer pointed to, will
/// be initialized.
/// - From the construction of a [`NeedsInit<'init, T>`] until the end of
/// `'init` it assumes full control over the pointee. This means that no one
/// else is allowed to access the underlying value.
///
/// [`Deref`]: core::ops::Deref
/// [`DerefMut`]: core::ops::DerefMut
#[repr(transparent)]
pub struct NeedsInit<'init, T: ?Sized> {
    inner: &'init mut T,
}

#[cfg(not(pinned_init_unsafe_no_enforce_init))]
impl<'init, T: ?Sized> Drop for NeedsInit<'init, T> {
    #[inline]
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
                    // SAFETY: this function does not exist and will generate a
                    // link error
                    trigger();
                }
            }
        }
    }
}

impl<'init, T> NeedsInit<'init, StaticUninit<T, false>> {
    /// Construct a new `NeedsInit` from the given pointer.
    ///
    /// # Safety
    ///
    /// - When the `'init` lifetime expires, the value at `inner` will be
    /// initialized and have changed type to `StaticUninit<T, true>`.
    /// - From the moment this function is called until the end of `'init` the
    /// produced [`NeedsInit`] becomes the only valid way to access the
    /// underlying value.
    /// - The caller needs to guarantee, that the pointer from which `inner` was
    /// derived changes its pointee type to `StaticUninit<T, true>`, when `'init` ends.
    #[inline]
    pub unsafe fn new_unchecked(inner: &'init mut StaticUninit<T, false>) -> Self {
        Self { inner }
    }

    /// Initializes the value behind this pointer with the supplied value
    #[inline]
    pub fn init(self, value: T) {
        unsafe {
            // SAFETY: We have been constructed by [`NeedsInit::new_unchecked`]
            // and thus have full control over the pointee, we now change its
            // type and never access it again (we are consumed). When `'init`
            // expires all accesses to the pointee will be made through
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
