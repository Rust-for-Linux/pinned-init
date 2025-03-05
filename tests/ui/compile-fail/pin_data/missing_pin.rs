extern crate pin_init;
use pin_init::*;

#[pin_data]
struct Foo {
    a: usize,
}

impl Foo {
    fn new(a: impl PinInit<usize>) -> impl PinInit<Self> {
        pin_init!(Self {
            a <- a,
        })
    }
}

fn main() {}
