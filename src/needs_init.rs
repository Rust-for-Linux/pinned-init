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
#[repr(transparent)]
pub struct NeedsPinnedInit<'init, T: ?Sized> {
    inner: Option<Pin<&'init mut T>>,
}

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
