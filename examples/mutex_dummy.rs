#![feature(
    type_alias_impl_trait,
    never_type,
    stmt_expr_attributes,
    raw_ref_op,
    new_uninit
)]

use core::{cell::UnsafeCell, marker::PhantomPinned, mem::MaybeUninit, pin::Pin};
use simple_safe_init::*;

#[derive(Debug)]
#[repr(C)]
struct mutex {
    data: [u8; 0],
}

unsafe extern "C" fn __init_mutex(_mutex: *mut mutex) {}

fn init_raw_mutex() -> impl PinInitializer<MaybeUninit<UnsafeCell<mutex>>, !> {
    let init = move |place: *mut MaybeUninit<UnsafeCell<mutex>>| {
        // SAFETY: place is valid
        unsafe { __init_mutex(place.cast()) };
        Ok(())
    };
    // SAFETY: the closure initializes all fields
    unsafe { PinInit::from_closure(init) }
}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

#[derive(Debug)]
pub struct Mutex<T: ?Sized> {
    raw: MaybeUninit<UnsafeCell<mutex>>,
    pin: PhantomPinned,
    val: UnsafeCell<T>,
}

fn create_single_mutex() {
    let mtx: Result<Pin<Box<Mutex<String>>>, AllocInitErr<!>> = Box::pin_init(pin_init! {
    Mutex<String> {
        raw: init_raw_mutex(),
        pin: PhantomPinned,
        val: UnsafeCell::new("Hello World".to_owned()),
    }});
    println!("{:?}", mtx);
}

#[derive(Debug)]
struct MultiMutex {
    data1: Mutex<String>,
    data2: Mutex<(u64, f64)>,
}

impl<T> Mutex<T> {
    const fn new(value: T) -> impl PinInitializer<Self, !> {
        let init = pin_init! { Self {
            raw: init_raw_mutex(),
            pin: PhantomPinned,
            val: UnsafeCell::new(value),
        }};
        init
    }
}

fn create_multi_mutex() {
    let mmx: Result<Pin<Box<MultiMutex>>, AllocInitErr<!>> = Box::pin_init(pin_init! {
    MultiMutex {
        data1: Mutex::new("Hello World".to_owned()),
        data2: Mutex::new((42, 13.37)),
    }});
    println!("{:?}", mmx);
}

fn main() {
    create_single_mutex();
    create_multi_mutex();
}
