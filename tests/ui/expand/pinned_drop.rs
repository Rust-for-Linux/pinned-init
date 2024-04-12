use core::{marker::PhantomPinned, pin::Pin};
use pinned_init::*;

#[pin_data(PinnedDrop)]
struct Foo {
    array: [u8; 1024 * 1024],
    #[pin]
    _pin: PhantomPinned,
}

#[pinned_drop]
impl PinnedDrop for Foo {
    fn drop(self: Pin<&mut Self>) {}
}
