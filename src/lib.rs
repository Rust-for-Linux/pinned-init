//! This library is used to facilitate safe pinned initialization for as many
//! parties involved as possible.
//! The process of initialization follows the following steps:
//! 1. You create a `FooUninit<...>` (an alias for `Foo<..., false>`)
//! 2. You move the `FooUninit<...>` behind a unique owning pointer (here named
//!     `SomePtr<FooUninit<...>>`)
//! 3. You use the `init()` function of `SomePtr<FooUninit<...>>` provided by
//!     the `SafeInit` trait, to initialize the contents
//! 3a. This library creates a `NeedsPinnedInit<FooUninit<...>>` from the
//!    `SomePtr<FooUninit<...>>`.
//! 3b. This library uses the `NeedsPinnedInit<FooUninit<...>>` to call the
//!     init_raw function of `FooUninit<...>` which initializes it.
//! 3c. This library transmutes the `SomePtr<FooUninit<...>>` to
//!     `SomePtr<Foo<...>>` and returns that.
//!
//! This library aims to make it ergonomic and safe to not only write code using
//! pinned initialized types, but also to create them.
//! When you embed a pinned init type within another struct, you can use the
//! `#[pinned_init]` macro to turn the outer struct into a pin initialized type and
//! automatically initializes the inner type.
//!
//! When you have more compilcated requirements (for example you need a self
//! referential struct) you probably cannot avoit writing unsafe code, but the
//! pinned initialization part will probably not require any unsafe if you
//! follow these guidelines:
//! 1. use `#[manual_init]` to add the minimal comapability for this libarary,
//! this will:
//! - put `#[pin_project]` your struct, you will have to manually add `#[pin]`
//! whenever you desire structual pinning
//! - derive some traits from this lib: transmute and manualinithelper
//! 2. implement PinnedInit for your type and use the begin_init() function of
//!    NeedsPinnedInit to access the fields you need for initialization, when you
//!    need access to other fields of the type, then TODO
#![no_std]
#![feature(generic_associated_types)]
#![deny(unsafe_op_in_unsafe_fn, missing_docs)]
use crate::{
    needs_init::NeedsPinnedInit, private::BeginInit, ptr::OwnedUniquePtr, transmute::TransmuteInto,
};
use core::pin::Pin;

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
pub mod static_uninit;

pub use pinned_init_macro::{manual_init, pinned_init};

#[doc(hidden)]
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{
        manual_init,
        needs_init::{NeedsInit, NeedsPinnedInit},
        pinned_init, PinnedInit, SafePinnedInit,
    };
}

#[doc(hidden)]
pub mod __private {
    pub use pin_project::pin_project;
}


#[doc(hidden)]
pub mod private {
    use core::pin::Pin;

    /// Trait implemented by the [`pinned_init`] and the [`manual_init`] proc
    /// macros. This trait should not be implemented manually.
    pub trait BeginInit {
        #[doc(hidden)]
        type OngoingInit<'init>: 'init
        where
            Self: 'init;
        #[doc(hidden)]
        unsafe fn __begin_init<'init>(self: Pin<&'init mut Self>) -> Self::OngoingInit<'init>
        where
            Self: 'init;
    }
}

/// Initializing a value in place (because it is pinned) requires a
/// transmuation. Because transmuations are inherently unsafe, this module aims
/// to create a safer abstraction and requires users to explicitly opt in to use
/// this initialization.
pub mod transmute {
    use core::{mem, pin::Pin};

    /// Marks and allows easier unsafe transmutation between types.
    /// When implementing this type manually, which should only be done for
    /// custom pointer types which also implement
    /// [`crate::ptr::OwnedUniquePtr`].
    /// When you use the proc macro attributes [`pinned_init`] and
    /// [`manual_init`] this trait will be implemented automatically to
    /// transmute from the uninitialized to the initialized variant.
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
    /// When implementing this type the following conditions must be true:
    /// - `T` and `Self` have the same layout.
    /// - transmutation is only sound, if all invariants of `T` are satisfied by
    /// all values of `Self` transmuted with this trait.
    pub unsafe trait TransmuteInto<T>: Sized {
        /// Unsafely transmutes `self` to `T`
        ///
        /// # Safety
        ///
        /// - `T` and `Self` must have the same layout.
        /// - All invariants of `T` need to be satisfied by `self`.
        unsafe fn transmute(self) -> T {
            unsafe {
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

    unsafe impl<T, U> TransmuteInto<Pin<U>> for Pin<T>
    where
        T: TransmuteInto<U>,
    {
        unsafe fn transmute_ptr(this: *const Self) -> *const Pin<U> {
            unsafe { mem::transmute(this) }
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
/// [`BeginInit`] for your struct, as implementing that trait is not supposed to
/// be done manually.
pub trait PinnedInit: TransmuteInto<Self::Initialized> + BeginInit {
    /// The initialized version of `Self`. `Self` can be transmuted via
    /// [`TransmuteInto`] into this type.
    type Initialized;

    /// Initialize the value behind the given pointer, this pointer ensures,
    /// that `Self` really will be initialized.
    fn init_raw(this: NeedsPinnedInit<Self>);
}

mod sealed {
    use super::*;

    pub trait Sealed<T: PinnedInit> {}

    impl<T: PinnedInit, P: OwnedUniquePtr<T>> Sealed<T> for Pin<P> where
        Pin<P>: TransmuteInto<Pin<P::Ptr<T::Initialized>>>
    {
    }
}

/// Sealed trait to facilitate safe initialization of the types supported by
/// this crate.
///
/// Use this traits [`Self::init`] method to initialize the T contained in `self`.
/// This trait is implemented only for [`Pin<P>`] `where P:` [`OwnedUniquePtr<T>`] `, T:` [`PinnedInit`].
pub trait SafePinnedInit<T: PinnedInit>:
    sealed::Sealed<T> + TransmuteInto<Self::Initialized> + Sized
{
    /// The type that represents the initialized version of `self`.
    type Initialized;

    /// Initialize the contents of `self`.
    fn init(self) -> Self::Initialized;
}

impl<T: PinnedInit, P: OwnedUniquePtr<T>> SafePinnedInit<T> for Pin<P>
where
    Pin<P>: TransmuteInto<Pin<P::Ptr<T::Initialized>>>,
{
    type Initialized = Pin<P::Ptr<T::Initialized>>;

    fn init(mut self) -> Self::Initialized {
        unsafe {
            // SAFETY: `self` implements `OwnedPinnedPtr`, thus giving us unique
            // access to the data behind `self`. Because we call `T::init_raw`
            // and `Self::transmute` below, the contract of
            // `NeedsPinnedInit::new_unchecked` is fullfilled (all pointers to
            // the data are aware of the new_unchecked call).
            let this = NeedsPinnedInit::new_unchecked(self.as_mut());
            T::init_raw(this);
            Self::transmute(self)
        }
    }
}
