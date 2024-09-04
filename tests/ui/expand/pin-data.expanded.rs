use core::marker::PhantomPinned;
use pinned_init::*;
struct Foo {
    array: [u8; 1024 * 1024],
    _pin: PhantomPinned,
}
const _: () = {
    struct __ThePinData {
        __phantom: ::core::marker::PhantomData<fn(Foo) -> Foo>,
    }
    impl ::core::clone::Clone for __ThePinData {
        fn clone(&self) -> Self {
            *self
        }
    }
    impl ::core::marker::Copy for __ThePinData {}
    #[allow(dead_code)]
    #[expect(clippy::missing_safety_doc)]
    impl __ThePinData {
        unsafe fn _pin<E>(
            self,
            slot: *mut PhantomPinned,
            init: impl ::pinned_init::PinInit<PhantomPinned, E>,
        ) -> ::core::result::Result<(), E> {
            unsafe { ::pinned_init::PinInit::__pinned_init(init, slot) }
        }
        unsafe fn array<E>(
            self,
            slot: *mut [u8; 1024 * 1024],
            init: impl ::pinned_init::Init<[u8; 1024 * 1024], E>,
        ) -> ::core::result::Result<(), E> {
            unsafe { ::pinned_init::Init::__init(init, slot) }
        }
    }
    unsafe impl ::pinned_init::__internal::HasPinData for Foo {
        type PinData = __ThePinData;
        unsafe fn __pin_data() -> Self::PinData {
            __ThePinData {
                __phantom: ::core::marker::PhantomData,
            }
        }
    }
    unsafe impl ::pinned_init::__internal::PinData for __ThePinData {
        type Datee = Foo;
    }
    #[allow(dead_code)]
    struct __Unpin<'__pin> {
        __phantom_pin: ::core::marker::PhantomData<fn(&'__pin ()) -> &'__pin ()>,
        __phantom: ::core::marker::PhantomData<fn(Foo) -> Foo>,
        _pin: PhantomPinned,
    }
    #[doc(hidden)]
    impl<'__pin> ::core::marker::Unpin for Foo
    where
        __Unpin<'__pin>: ::core::marker::Unpin,
    {}
    trait MustNotImplDrop {}
    #[expect(drop_bounds)]
    impl<T: ::core::ops::Drop> MustNotImplDrop for T {}
    impl MustNotImplDrop for Foo {}
    #[expect(non_camel_case_types)]
    trait UselessPinnedDropImpl_you_need_to_specify_PinnedDrop {}
    impl<
        T: ::pinned_init::PinnedDrop,
    > UselessPinnedDropImpl_you_need_to_specify_PinnedDrop for T {}
    impl UselessPinnedDropImpl_you_need_to_specify_PinnedDrop for Foo {}
};
