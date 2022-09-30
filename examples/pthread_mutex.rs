// inspired by https://github.com/nbdd0121/pin-init/blob/trunk/examples/pthread_mutex.rs
use core::{
    cell::UnsafeCell,
    marker::PhantomPinned,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    pin::Pin,
};
use simple_safe_init::*;
use std::{
    sync::Arc,
    thread::{sleep, Builder},
    time::Duration,
};

pub struct PThreadMutex<T> {
    raw: UnsafeCell<libc::pthread_mutex_t>,
    data: UnsafeCell<T>,
    pin: PhantomPinned,
}

unsafe impl<T: Send> Send for PThreadMutex<T> {}
unsafe impl<T: Send> Sync for PThreadMutex<T> {}

impl<T> Drop for PThreadMutex<T> {
    fn drop(&mut self) {
        unsafe { libc::pthread_mutex_destroy(self.raw.get()) };
    }
}

#[derive(Debug)]
pub struct Error(std::io::Error);

impl From<core::convert::Infallible> for Error {
    fn from(e: core::convert::Infallible) -> Self {
        match e {}
    }
}

impl<T> PThreadMutex<T> {
    pub fn new(data: T) -> impl PinInit<Self, Error> {
        fn init_raw() -> impl PinInit<UnsafeCell<libc::pthread_mutex_t>, Error> {
            let init = |slot: *mut UnsafeCell<libc::pthread_mutex_t>| {
                // we can cast, because `UnsafeCell` has the same layout as T.
                let slot: *mut libc::pthread_mutex_t = slot.cast();
                let mut attr = MaybeUninit::uninit();
                let attr = attr.as_mut_ptr();
                // SAFETY: ptr is valid
                let ret = unsafe { libc::pthread_mutexattr_init(attr) };
                if ret != 0 {
                    return Err(Error(std::io::Error::from_raw_os_error(ret)));
                }
                // SAFETY: attr is initialized
                let ret =
                    unsafe { libc::pthread_mutexattr_settype(attr, libc::PTHREAD_MUTEX_NORMAL) };
                if ret != 0 {
                    // SAFETY: attr is initialized
                    unsafe { libc::pthread_mutexattr_destroy(attr) };
                    return Err(Error(std::io::Error::from_raw_os_error(ret)));
                }
                // SAFETY: slot is valid
                unsafe { slot.write(libc::PTHREAD_MUTEX_INITIALIZER) };
                // SAFETY: attr and slot are valid ptrs and attr is initialized
                let ret = unsafe { libc::pthread_mutex_init(slot, attr) };
                // SAFETY: attr was initialized
                unsafe { libc::pthread_mutexattr_destroy(attr) };
                if ret != 0 {
                    return Err(Error(std::io::Error::from_raw_os_error(ret)));
                }
                Ok(())
            };
            // SAFETY: mutex has been initialized
            unsafe { PinInitClosure::from_closure(init) }
        }
        pin_init!(Self {
            data: UnsafeCell::new(data),
            raw: init_raw(),
            pin: PhantomPinned,
        })
    }

    pub fn lock(&self) -> PThreadMutexGuard<'_, T> {
        // SAFETY: raw is always initialized
        unsafe { libc::pthread_mutex_lock(self.raw.get()) };
        PThreadMutexGuard { mtx: self }
    }
}

pub struct PThreadMutexGuard<'a, T> {
    mtx: &'a PThreadMutex<T>,
}

impl<'a, T> Drop for PThreadMutexGuard<'a, T> {
    fn drop(&mut self) {
        // SAFETY: raw is always initialized
        unsafe { libc::pthread_mutex_unlock(self.mtx.raw.get()) };
    }
}

impl<'a, T> Deref for PThreadMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mtx.data.get() }
    }
}

impl<'a, T> DerefMut for PThreadMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mtx.data.get() }
    }
}

fn main() {
    let mtx: Pin<Arc<PThreadMutex<usize>>> = Arc::pin_init(PThreadMutex::new(0)).unwrap();
    let mut handles = vec![];
    let thread_count = 20;
    let workload = 1000_000;
    for i in 0..thread_count {
        let mtx = mtx.clone();
        handles.push(
            Builder::new()
                .name(format!("worker #{i}"))
                .spawn(move || {
                    for _ in 0..workload {
                        *mtx.lock() += 1;
                    }
                    println!("{i} halfway");
                    sleep(Duration::from_millis((i as u64) * 10));
                    for _ in 0..workload {
                        *mtx.lock() += 1;
                    }
                    println!("{i} finished");
                })
                .expect("should not fail"),
        );
    }
    for h in handles {
        h.join().expect("thread paniced");
    }
    println!("{:?}", &*mtx.lock());
    assert_eq!(*mtx.lock(), workload * thread_count * 2);
}
