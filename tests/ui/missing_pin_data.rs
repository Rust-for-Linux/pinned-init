#![feature(allocator_api)]
extern crate pinned_init;
use pinned_init::*;

struct Foo {
    a: usize,
}

impl Foo {
    fn new() -> impl PinInit<Self> {
        pin_init!(Self { a: 42 })
    }
}

fn main() {}
