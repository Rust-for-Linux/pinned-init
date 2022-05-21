#![feature(generic_associated_types, const_ptr_offset_from, const_refs_to_cell)]
#![deny(unsafe_op_in_unsafe_fn)]

use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};
use pinned_init::prelude::*;

// imagine this is some FFI struct
struct RawMutex;

impl RawMutex {
    fn new() -> RawMutex {
        RawMutex
    }
    unsafe fn init(_: *mut RawMutex) {}
    unsafe fn lock(_: *mut RawMutex) {}
    unsafe fn unlock(_: *mut RawMutex) {}
}

impl<T> MutexUninit<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: MaybeUninit::uninit(),
            value: UnsafeCell::new(value),
        }
    }
}

#[manual_init(pinned)]
pub struct Mutex<T> {
    #[pin]
    #[init]
    #[uninit = MaybeUninit::<UnsafeCell<RawMutex>>]
    inner: UnsafeCell<RawMutex>,
    value: UnsafeCell<T>,
}

impl<T> PinnedInit for MutexUninit<T> {
    type Initialized = Mutex<T>;
    type Param = ();

    fn init_raw(this: NeedsPinnedInit<Self>, _: ()) {
        let MutexOngoingInit { mut inner, .. } = this.begin_init();
        unsafe {
            // SAFETY: FFI call initializes the raw mutex
            let ptr = inner.as_ptr_mut();
            (*ptr).write(UnsafeCell::new(RawMutex::new()));
            RawMutex::init((*ptr).assume_init_mut().get());
            inner.assume_init();
        }
    }
}

pub struct Guard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> Deref for Guard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            // SAFETY: valid pointer and we own the mutex
            &*self.mutex.value.get()
        }
    }
}

impl<'a, T> DerefMut for Guard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            // SAFETY: valid pointer and we own the mutex
            &mut *self.mutex.value.get()
        }
    }
}

impl<'a, T> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: FFI call on valid pointer
            RawMutex::unlock(self.mutex.inner.get());
        }
    }
}

impl<T> Mutex<T> {
    pub fn lock(&self) -> Guard<'_, T> {
        unsafe {
            // SAFETY: FFI call on valid pointer
            RawMutex::lock(self.inner.get());
        }
        Guard { mutex: self }
    }
}

fn main() {
    let mutex = Box::pin(MutexUninit::new("Hello World".to_owned())).init();
    *mutex.lock() = "hey".to_owned();
    println!("{}", &*mutex.lock());
}
