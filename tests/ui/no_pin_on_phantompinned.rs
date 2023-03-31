#![no_std]
#![feature(allocator_api)]
extern crate pinned_init;
use core::marker::{self, PhantomPinned};
use pinned_init::*;

#[pin_data]
struct Foo {
    pin1: PhantomPinned,
    pin2: marker::PhantomPinned,
    pin3: core::marker::PhantomPinned,
    pin4: ::core::marker::PhantomPinned,
}

fn main() {}
