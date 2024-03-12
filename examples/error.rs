#![feature(allocator_api)]

use core::convert::Infallible;

#[derive(Debug)]
pub struct AllocError;

impl From<Infallible> for AllocError {
    fn from(_: Infallible) -> Self {
        Self
    }
}

impl From<core::alloc::AllocError> for AllocError {
    fn from(_: core::alloc::AllocError) -> Self {
        Self
    }
}

fn main() {}
