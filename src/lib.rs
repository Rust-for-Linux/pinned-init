#![doc = include_str!("lib.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![feature(generic_associated_types)]
#![deny(unsafe_op_in_unsafe_fn, missing_docs)]
use crate::{
    needs_init::{NeedsInit, NeedsPinnedInit},
    private::{BeginInit, BeginPinnedInit},
    ptr::OwnedUniquePtr,
    transmute::TransmuteInto,
};
use core::pin::Pin;

#[cfg(feature = "alloc")]
extern crate alloc;

macro_rules! if_cfg {
    (if $cfg:tt {$($body:tt)*} else {$($els:tt)*}) => {
        #[cfg $cfg]
        {
            $($body)*
        }
        #[cfg(not $cfg)]
        {
            $($els)*
        }
    };
}

pub mod needs_init;
pub mod ptr;

/// Use this attribute on a struct with named fields to ensure safe
/// pinned initialization of all the fields marked with `#[init]`.
///
/// This attribute does several things, it:
/// - `#[pin_project]`s your struct, structually pinning all fields with `#[init]` implicitly (adding `#[pin]`).
/// - adds a constant type parameter of type bool with a default value of true.
/// This constant type parameter indicates if your struct is in an initialized
/// (and thus also pinned) state. A type alias `{your-struct-name}Uninit` is
/// created to refer to the uninitialized variant more ergonomically, it should
/// always be used instead of specifying the const parameter.
/// - propagates that const parameter to all fields marked with `#[init]`.
/// - implements [`PinnedInit`] for your struct delegating to all fields marked
/// with `#[init]`.
/// - implements [`TransmuteInto<{your-struct-name}>`]()
/// `for`{your-struct-name}Uninit` and checks for layout equivalence between the
/// two.
/// - creates a custom type borrowing from your struct that is used as the
/// `OngoingInit` type for the [`BeginPinnedInit`] trait.
/// - implements [`BeginPinnedInit`] for your struct.
///
/// Then you can safely, soundly and ergonomically initialize a value of such a
/// struct behind an [`OwnedUniquePtr<{your-struct-name}>`]:
/// TODO example
pub use pinned_init_macro::manual_init;

/// Use this attribute on a struct with named fields to ensure safer
/// pinned initialization of all the fields marked with `#[init]`.
///
/// This attribute does several things, it:
/// - `#[pin_project]`s your struct, structually pinning all fields with `#[pin]`.
/// - adds a constant type parameter of type bool with a default value of true.
/// This constant type parameter indicates if your struct is in an initialized
/// (and thus also pinned) state. A type alias `{your-struct-name}Uninit` is
/// created to refer to the uninitialized variant more ergonomically, it should
/// always be used instead of specifying the const parameter.
/// - propagates that const parameter to all fields marked with `#[init]`.
/// - implements [`TransmuteInto<{your-struct-name}>`]
/// `for`{your-struct-name}Uninit` and checks for layout equivalence between the
/// two.
/// - creates a custom type borrowing from your struct that is used as the
/// `OngoingInit` type for the [`BeginPinnedInit`] trait.
/// - implements [`BeginPinnedInit`] for your struct.
///
/// The only thing you need to implement is [`PinnedInit`].
///
/// Then you can safely, soundly and ergonomically initialize a value of such a
/// struct behind an [`OwnedUniquePtr<{your-struct-name}>`]:
/// TODO example
pub use pinned_init_macro::pinned_init;

#[doc(hidden)]
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{
        manual_init,
        needs_init::{NeedsInit, NeedsPinnedInit},
        pinned_init, Init, PinnedInit, SafePinnedInit,
    };
}

#[doc(hidden)]
pub mod __private {
    pub use pin_project::pin_project;
}

#[doc(hidden)]
pub mod private {
    use core::pin::Pin;

    pub use pinned_init_macro::{BeginInit, BeginPinnedInit};

    /// Trait implemented by the [`pinned_init`] and the [`manual_init`] proc
    /// macros. This trait should not be implemented manually.
    ///
    /// [`pinned_init`]: crate::pinned_init
    /// [`manual_init`]: crate::manual_init
    pub trait BeginPinnedInit {
        #[doc(hidden)]
        type OngoingInit<'init>: 'init
        where
            Self: 'init;

        /// api internal function, do not call from outside this library!
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
        #[doc(hidden)]
        unsafe fn __begin_init<'init>(self: Pin<&'init mut Self>) -> Self::OngoingInit<'init>
        where
            Self: 'init;
    }

    /// Trait implemented by the [`pinned_init`] and the [`manual_init`] proc
    /// macros. This trait should not be implemented manually.
    ///
    /// [`pinned_init`]: crate::pinned_init
    /// [`manual_init`]: crate::manual_init
    pub trait BeginInit {
        #[doc(hidden)]
        type OngoingInit<'init>: 'init
        where
            Self: 'init;

        /// api internal function, do not call from outside this library!
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
        #[doc(hidden)]
        unsafe fn __begin_init<'init>(self: &'init mut Self) -> Self::OngoingInit<'init>
        where
            Self: 'init;
    }

    /// Marks types that have an uninitialized form, automatically implemented by [`manual_init`]
    /// and [`pinned_init`].
    ///
    /// [`pinned_init`]: crate::pinned_init
    /// [`manual_init`]: crate::manual_init
    pub unsafe trait AsUninit: Sized {
        /// Uninitialized form of `Self`.
        type Uninit: crate::transmute::TransmuteInto<Self>;
    }
}

/// Initializing a value in place (because it is pinned) requires a
/// transmuation. Because transmuations are inherently unsafe, this module aims
/// to create a safer abstraction and requires users to explicitly opt in to use
/// this initialization.
pub mod transmute {
    use core::{mem, pin::Pin};

    /// Marks and allows easier unsafe transmutation between types.
    /// This trait should **not** be implemented manually.
    ///
    /// When implementing this type manually (despite being told not to!), you
    /// must ensure, that `Self` is indeed transmutible to `T`, review the
    /// current [unsafe code guidelines]()
    /// to ensure that `Self` and `T` will also be transmutible in future
    /// versions of the compiler.
    ///
    /// When you use the proc macro attributes [`pinned_init`] and
    /// [`manual_init`] this trait will be implemented automatically to
    /// transmute from the uninitialized to the initialized variant.
    /// This is accompanied with static compile checks, that the layout of both
    /// types is the same.
    ///
    /// When using this trait, it is not required to use one of the provided
    /// functions, you may [`mem::transmute`] a value of `Self` to `T`, you may
    /// use a union to reinterpret a `Self` as a `T` and you may use pointer
    /// casts to cast a pointer to `Self` to a pointer to `T`.
    /// Of course you will need to still ensure the safety requirements of this
    /// trait.
    ///
    /// # Safety
    ///
    /// When implementing this trait the following conditions must be true:
    /// - `T` and `Self` have the same layout, compiler version updates must not
    /// break this invariant.
    /// - transmutation is only sound, if all invariants of `T` are satisfied by
    /// all values of `Self` transmuted with this trait.
    ///
    /// Again: **DO NOT IMPLEMENT THIS MANUALLY** as this requires very strict
    /// invariants to be upheld, that concern layout of types. The behaviour of
    /// the compiler has not yet been fully specified, so you need to take extra
    /// care to ensure future compiler compatiblity.
    ///
    /// [`pinned_init`]: crate::pinned_init
    /// [`manual_init`]: crate::manual_init
    pub unsafe trait TransmuteInto<T>: Sized {
        /// Unsafely transmutes `self` to `T`.
        ///
        /// # Safety
        ///
        /// - `T` and `Self` must have the same layout.
        /// - All invariants of `T` need to be satisfied by `self`.
        #[inline]
        unsafe fn transmute(self) -> T {
            unsafe {
                // SAFETY: the invariants between `transmute` and `transmute_ptr` are the same
                let ptr = &self as *const Self;
                mem::forget(self);
                let ptr = Self::transmute_ptr(ptr);
                ptr.read()
            }
        }

        /// Unsafely transmutes a pointer to `Self` to a pointer to `T`.
        ///
        /// # Safety
        ///
        /// - `T` and `Self` must have the same layout.
        /// - All invariants of `T` need to be satisfied by the value at `this`.
        unsafe fn transmute_ptr(this: *const Self) -> *const T;
    }

    // SAFETY: [`Pin`] is `repr(transparent)`, thus permitting transmutations between
    // `Pin<P> <-> P`. Because T permits transmuting `T -> U`, transmuting
    // `Pin<T> -> Pin<U>` is also permitted (effectively `Pin<T> -> T -> U
    // ->`Pin<U>`).
    unsafe impl<T, U> TransmuteInto<Pin<U>> for Pin<T>
    where
        T: TransmuteInto<U>,
    {
        #[inline]
        unsafe fn transmute_ptr(this: *const Self) -> *const Pin<U> {
            unsafe {
                // SAFETY: `T: TransmuteInto<U>` guarantees that we can
                // transmute `T -> U`. The caller needs to guarantee, that the
                // invariants of `U` are upheld when this `T` is transmuted to
                // `U`.
                mem::transmute(this)
            }
        }
    }
}

/// Facilitates pinned initialization.
/// Before you implement this trait manually, look at the [`pinned_init`] proc
/// macro attribute, it can be used to implement this trait in a safe and sound
/// fashion in many cases.
///
/// You will need to implement this trait yourself, if your struct contains any
/// fields with the [`static_uninit::StaticUninit`] type. When implementing this
/// trait manually, use the [`manual_init`] proc macro attribute to implement
/// [`BeginPinnedInit`] for your struct, as implementing that trait is not supposed to
/// be done manually.
pub trait PinnedInit: TransmuteInto<Self::Initialized> + BeginPinnedInit {
    /// The initialized version of `Self`. `Self` can be transmuted via
    /// [`TransmuteInto`] into this type.
    type Initialized;
    /// An optional Parameter used to initialize `Self`.
    /// When you do not need it, set to `()`
    type Param;

    /// Initialize the value behind the given pointer with the given parameter, this pointer ensures,
    /// that `Self` really will be initialized.
    fn init_raw(this: NeedsPinnedInit<Self>, param: Self::Param);
}

/// Facilitates pinned initialization.
/// Before you implement this trait manually, look at the [`pinned_init`] proc
/// macro attribute, it can be used to implement this trait in a safe and sound
/// fashion in many cases.
///
/// You will need to implement this trait yourself, if your struct contains any
/// fields with the [`static_uninit::StaticUninit`] type. When implementing this
/// trait manually, use the [`manual_init`] proc macro attribute to implement
/// [`BeginPinnedInit`] for your struct, as implementing that trait is not supposed to
/// be done manually.
pub trait Init: TransmuteInto<Self::Initialized> + BeginInit {
    /// The initialized version of `Self`. `Self` can be transmuted via
    /// [`TransmuteInto`] into this type.
    type Initialized;
    /// An optional Parameter used to initialize `Self`.
    /// When you do not need it, set to `()`
    type Param;

    /// Initialize the value behind the given pointer with the given parameter, this pointer ensures,
    /// that `Self` really will be initialized.
    fn init_raw(this: NeedsInit<Self>, param: Self::Param);
}

// used to prevent accidental/mailicious implementations of `SafePinnedInit`
mod sealed {
    use super::*;

    pub trait Sealed<T: PinnedInit> {}

    impl<T: PinnedInit, P: OwnedUniquePtr<T>> Sealed<T> for Pin<P> {}
}

/// Sealed trait to facilitate safe initialization of the types supported by
/// this crate.
///
/// Use this traits [`Self::init`] method to initialize the T contained in `self`.
/// This trait is implemented only for [`Pin<P>`] `where P:` [`OwnedUniquePtr<T>`] `, T:` [`PinnedInit`].
pub trait SafePinnedInit<T: PinnedInit>: sealed::Sealed<T> + Sized {
    /// The type that represents the initialized version of `self`.
    type Initialized;

    /// Initialize the contents of `self`.
    #[inline]
    fn init(self) -> Self::Initialized
    where
        (): Into<T::Param>,
    {
        self.init_with(().into())
    }

    /// Initialize the contents of `self`.
    fn init_with(self, param: T::Param) -> Self::Initialized;
}

impl<T: PinnedInit, P: OwnedUniquePtr<T>> SafePinnedInit<T> for Pin<P> {
    type Initialized = Pin<P::Ptr<T::Initialized>>;

    #[inline]
    fn init_with(mut self, param: T::Param) -> Self::Initialized {
        unsafe {
            // SAFETY: `self` implements `OwnedUniquePtr`, thus giving us unique
            // access to the data behind `self`. Because we call `T::init_raw`
            // and `P::transmute_pointee_pinned` below, the contract of
            // `NeedsPinnedInit::new_unchecked` is fullfilled (all pointers to
            // the data are aware of the new_unchecked call).
            let this = NeedsPinnedInit::new_unchecked(self.as_mut());
            T::init_raw(this, param);
            P::transmute_pointee_pinned(self)
        }
    }
}
