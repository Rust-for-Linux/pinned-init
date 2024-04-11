use pinned_init::*;
use std::pin::Pin;

#[pin_data(PinnedDrop)]
struct Foo<T> {
    t: T,
}

#[pinned_drop]
impl<T> PinnedDrop<T> for Foo<T> {
    fn drop(self: Pin<&mut Self>) {}
}

fn main() {}
