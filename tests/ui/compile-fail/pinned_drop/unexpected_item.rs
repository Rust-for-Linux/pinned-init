use pin_init::*;

#[pin_data(PinnedDrop)]
struct Foo {}

#[pinned_drop]
impl PinnedDrop for Foo {
    const BAZ: usize = 0;
}

fn main() {}
