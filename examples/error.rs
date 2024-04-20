#![allow(unused_attributes)]
#![feature(allocator_api)]

use core::convert::Infallible;
use std::alloc::AllocError;

#[derive(Debug)]
pub struct Error;

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        Self
    }
}

impl From<AllocError> for Error {
    fn from(_: AllocError) -> Self {
        Self
    }
}

#[allow(dead_code)]
fn main() {}
