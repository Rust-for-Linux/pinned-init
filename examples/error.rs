#![feature(allocator_api)]

use core::convert::Infallible;
use std::alloc::AllocError;

#[derive(Debug)]
pub struct Error;

impl From<Infallible> for Error {
    fn from(e: Infallible) -> Self {
        match e {}
    }
}

impl From<AllocError> for Error {
    fn from(_: AllocError) -> Self {
        Self
    }
}

fn main() {}
