use pinned_init::*;
use std::pin::Pin;

#[pin_data(PinnedDrop)]
struct Foo {}

#[pinned_drop]
unsafe impl PinnedDrop for Foo {
    fn drop(self: Pin<&mut Self>) {}
}

fn main() {}
