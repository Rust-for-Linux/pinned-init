#![feature(allocator_api)]
use pinned_init::*;
use std::alloc::AllocError;

struct Foo {
    a: Box<usize>,
    bar: Bar,
}

struct Bar {
    b: usize,
}

impl Foo {
    fn new() -> impl Init<Self, AllocError> {
        try_init!(Self {
            a: Box::new(42),
            bar <- init!(Bar { b: 42 }),
        }? AllocError)
    }
}

fn main() {}
