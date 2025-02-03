use pin_init::*;
use std::marker::{self, PhantomPinned};

#[pin_data]
struct Foo {
    pin1: PhantomPinned,
    pin2: marker::PhantomPinned,
    pin3: core::marker::PhantomPinned,
    pin4: ::core::marker::PhantomPinned,
}

fn main() {}
