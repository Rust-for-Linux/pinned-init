use core::marker::PhantomPinned;
use pin_init::*;

#[pin_data]
struct Foo {
    array: [u8; 1024 * 1024],
    #[pin]
    _pin: PhantomPinned,
}
