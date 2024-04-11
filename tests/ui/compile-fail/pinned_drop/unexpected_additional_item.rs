use pinned_init::*;

#[pin_data(PinnedDrop)]
struct Foo {}

#[pinned_drop]
impl PinnedDrop for Foo {
    fn drop(self: Pin<&mut Self>) {}

    const BAZ: usize = 0;
}

fn main() {}
