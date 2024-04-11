use pinned_init::*;

#[pin_data(PinnedDrop)]
struct Foo {}

#[pinned_drop]
impl PinnedDrop for Foo {}

fn main() {}
