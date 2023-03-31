#![feature(allocator_api)]
extern crate pinned_init;
use pinned_init::*;
use std::pin::Pin;

struct Foo {}

#[pinned_drop]
impl PinnedDrop for Foo {
    fn drop(self: Pin<&mut Self>) {}
}

fn main() {}
