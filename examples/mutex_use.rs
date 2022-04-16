#![deny(unsafe_op_in_unsafe_fn)]
#![feature(
    generic_associated_types,
    const_ptr_offset_from,
    const_refs_to_cell,
    generic_const_exprs
)]
#![feature(get_mut_unchecked)]
use core::pin::*;
use pinned_init::{prelude::*, *};

mod mutex;
mod unique;

use mutex::*;
use unique::*;

#[pinned_init]
struct ImportantData {
    #[init]
    msg: Mutex<String>,
    #[init]
    idx: Mutex<usize>,
    #[init]
    magic: Mutex<*const ()>,
}

impl ImportantData<false> {
    fn new(msg: &str, idx: usize, magic: fn()) -> Self {
        Self {
            msg: Mutex::new(msg.to_owned()),
            idx: Mutex::new(idx),
            magic: Mutex::new(magic as *const ()),
        }
    }
}

fn main() {
    let my_data: Pin<UniqueArc<ImportantData<false>>> =
        UniqueArc::pin(ImportantData::new("Hello mutex world!", 1, main));
    let my_data: Pin<UniqueArc<ImportantData>> = my_data.init();
    println!("{}", &*my_data.msg.lock());
}
