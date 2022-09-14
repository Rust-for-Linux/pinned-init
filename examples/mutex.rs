#![feature(
    type_alias_impl_trait,
    generic_associated_types,
    never_type,
    stmt_expr_attributes,
    raw_ref_op,
    proc_macro_hygiene,
    new_uninit
)]

use core::{cell::UnsafeCell, marker::PhantomPinned, pin::Pin};
use simple_safe_init::*;

#[derive(Debug)]
#[repr(C)]
struct mutex {
    data: [u8; 0],
}

extern "C" {
    fn __init_mutex(mutex: *mut mutex);
}

fn init_raw_mutex() -> impl Initializer<mutex, !> {
    let init = move |place: *mut mutex| {
        // SAFETY: place is valid
        unsafe { __init_mutex(place) };
        Ok(())
    };
    // SAFETY: the closure initializes all fields
    unsafe { Init::from_closure(init) }
}

#[derive(Debug)]
pub struct Mutex<T> {
    raw: mutex,
    pin: PhantomPinned,
    val: UnsafeCell<T>,
}

fn create_single_mutex() {
    let mtx: Result<Pin<Box<Mutex<String>>>, !> = Box::pin_init(
        #[init]
        Mutex::<String> {
            raw: init_raw_mutex(),
            pin: PhantomPinned,
            val: UnsafeCell::new("Hello World".to_owned()),
        },
    );
    println!("{:?}", mtx);
}

#[derive(Debug)]
struct MultiMutex {
    data1: Mutex<String>,
    data2: Mutex<(u64, f64)>,
}

impl<T> Mutex<T> {
    fn new(value: T) -> impl Initializer<Self, !> {
        #[init]
        Self {
            raw: init_raw_mutex(),
            pin: PhantomPinned,
            val: UnsafeCell::new(value),
        }
    }
}

fn create_multi_mutex() {
    let mmx: Result<Pin<Box<MultiMutex>>, !> = Box::pin_init(
        #[init]
        MultiMutex {
            data1: Mutex::new("Hello World".to_owned()),
            data2: Mutex::new((42, 13.37)),
        },
    );
    println!("{:?}", mmx);
}

fn main() {
    create_single_mutex();
    create_multi_mutex();
}
