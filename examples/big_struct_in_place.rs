use core::convert::Infallible;
use simple_safe_init::*;

// Struct with size over 1GiB
#[derive(Debug)]
pub struct BigStruct {
    buf: [u8; 1024 * 1024 * 1024],
    a: u64,
    b: u64,
    c: u64,
    d: u64,
    managed_buf: ManagedBuf,
}

#[derive(Debug)]
pub struct ManagedBuf {
    buf: [u8; 1024 * 1024],
}

impl ManagedBuf {
    pub fn new() -> impl Init<Self, Infallible> {
        init! { ManagedBuf {
            buf: zeroed(),
        }}
    }
}

fn main() {
    // we want to initialize the struct in place, otherwise we would get a stackoverflow
    let buf: Result<Box<BigStruct>, AllocInitErr<Infallible>> = Box::init(init! { BigStruct {
        buf: zeroed(),
        a: 7,
        b: 186,
        c: 7789,
        d: 34,
        managed_buf: ManagedBuf::new(),
    }});
    let buf = buf.unwrap();
    println!("{}", core::mem::size_of_val(&*buf));
}
