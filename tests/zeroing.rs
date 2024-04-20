#![allow(dead_code)]
use std::{marker::PhantomPinned, ptr::addr_of_mut};

use pinned_init::*;

const MARKS: usize = 64;

#[pin_data]
#[derive(Zeroable)]
pub struct Foo {
    buf: [u8; 1024 * 1024],
    marks: [*mut u8; MARKS],
    pos: usize,
    #[pin]
    _pin: PhantomPinned,
}

impl Foo {
    pub fn new() -> impl PinInit<Self> {
        pin_init!(&this in Self {
            marks: {
                let ptr = this.as_ptr();
                let ptr = unsafe { addr_of_mut!((*ptr).buf)}.cast::<u8>();
                [ptr; MARKS]},
            ..Zeroable::zeroed()
        })
    }
}

#[test]
fn test() {
    let _ = Box::pin_init(Foo::new()).unwrap();
}
