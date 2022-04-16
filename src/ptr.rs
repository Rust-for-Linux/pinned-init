//! Module providing a special pointer trait used to transfer owned data and to
//! allow safe transmuation of data without forgetting to change other pointer
//! types.

use core::ops::DerefMut;

mod sealed {
    #[doc(hidden)]
    pub struct Sealed;
}

#[doc(hidden)]
pub trait TypesEq<T> {
    fn __no_impls_outside_this_crate(_: sealed::Sealed);
}

#[doc(hidden)]
impl<T> TypesEq<T> for T {
    fn __no_impls_outside_this_crate(_: sealed::Sealed) {}
}

/// A (smart) unique pointer which owns its data (e.g. [`alloc::boxed::Box`]).
/// This pointer provides access to T via [`DerefMut`].
///
/// # Safety
///
/// All types implementing this trait need to
/// - own the data they point to.
/// - be the only way to access the data behind this pointer.
/// - provide the same pointer type as `Self` with only a different pointee via the
/// [`Self::Ptr`] associated type.
pub unsafe trait OwnedUniquePtr<T: ?Sized>: DerefMut<Target = T> + Sized
where
    Self: TypesEq<Self::Ptr<T>>,
{
    /// Access the same underlying pointer type with a different pointee type.
    /// `Self == Self::Ptr<T>`
    type Ptr<U: ?Sized>: DerefMut<Target = U>;
}

#[cfg(feature = "alloc")]
unsafe impl<T: ?Sized> OwnedUniquePtr<T> for alloc::boxed::Box<T> {
    type Ptr<U: ?Sized> = alloc::boxed::Box<U>;
}
