#![feature(allocator_api)]
extern crate pinned_init;
use pinned_init::*;

#[pin_data]
struct Foo {
    a: usize,
    b: usize,
}

impl Foo {
    fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            a: 0,
            b: {
                println!("{:?}", a);
                0
            }
        })
    }
}

fn main() {}
