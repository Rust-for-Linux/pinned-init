use core::marker::PhantomPinned;
use pinned_init::*;

#[pin_data]
struct Foo {
    array: [u8; 1024 * 1024],
    #[pin]
    _pin: PhantomPinned,
}
