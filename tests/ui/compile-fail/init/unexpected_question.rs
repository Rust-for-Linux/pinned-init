#![feature(allocator_api)]
#![allow(unused_attributes)]
#[path = "../../../../examples/mutex.rs"]
mod mutex;
use mutex::*;
use pinned_init::*;
use std::pin::Pin;

#[pin_data]
struct Foo {
    next: Option<Pin<Box<Foo>>>,
}

impl Foo {
    fn new(next: Option<impl PinInit<Self, Error>>) -> impl PinInit<Self, Error> {
        try_pin_init!(Self {
            next: next.map(|next| Box::try_pin_init(next)).transpose()?,
        }? Error)
    }
}

#[pin_data]
struct Bar {
    #[pin]
    foo: Foo,
}

impl Bar {
    fn new() -> impl PinInit<Self, Error> {
        try_pin_init!(Self {
            foo <- Foo::new(None)?,
        }? Error)
    }
}

fn main() {}
