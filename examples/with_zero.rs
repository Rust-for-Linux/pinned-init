#![feature(allocator_api, no_coverage)]
use pinned_init::*;

#[pin_data]
#[derive(Zeroable)]
pub struct Foo {
    a: usize,
    b: usize,
}

fn main() {
    let x = pin_init!(Foo {
        a: 0,
        ..Zeroable::zeroed()
    });
}
