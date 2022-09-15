#![feature(
    type_alias_impl_trait,
    never_type,
    try_blocks,
    stmt_expr_attributes,
    raw_ref_op,
    new_uninit,
    unwrap_infallible
)]
use core::{
    cell::{Cell, UnsafeCell},
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};
use std::{
    sync::Arc,
    thread::{current, park, sleep, Builder, Thread},
    time::Duration,
};

use simple_safe_init::*;
#[allow(unused_attributes)]
pub mod linked_list;
use linked_list::*;

macro_rules! debug {
    ($($t:tt)*) => {
        //print!($($t)*);
    };
}

macro_rules! debugln {
    ($($t:tt)*) => {
        //println!($($t)*);
    };
}

pub struct SpinLock {
    inner: AtomicBool,
}

impl SpinLock {
    pub fn acquire(&self) -> SpinLockGuard<'_> {
        while self
            .inner
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {}
        SpinLockGuard(self)
    }

    pub fn new() -> Self {
        Self {
            inner: AtomicBool::new(false),
        }
    }
}

pub struct SpinLockGuard<'a>(&'a SpinLock);

impl Drop for SpinLockGuard<'_> {
    fn drop(&mut self) {
        self.0.inner.store(false, Ordering::Release);
    }
}

pub struct Mutex<T> {
    wait_list: ListHead,
    spin_lock: SpinLock,
    locked: Cell<bool>,
    data: UnsafeCell<T>,
}

impl<T> Mutex<T> {
    pub fn new(val: T) -> impl PinInitializer<Self, !> {
        pin_init!(Self {
            wait_list: ListHead::new(),
            spin_lock: SpinLock::new(),
            locked: Cell::new(false),
            data: UnsafeCell::new(val),
        })
    }
    pub fn lock(&self) -> MutexGuard<'_, T> {
        let t = current();
        #[allow(unused)]
        let tname = t.name().unwrap_or("unnamed thread");
        debugln!("{tname}: [Mutex::lock] getting spinlock...");
        let mut sguard = self.spin_lock.acquire();
        debugln!("{tname}: [Mutex::lock] acquired spinlock");
        if self.locked.get() {
            Result::<(), !>::into_ok(
                try {
                    debugln!("{tname}: [Mutex::lock] adding wait_entry");
                    debug!("{tname}: [Mutex::lock] list status: ");
                    unsafe {
                        ListHead::debug_print(NonNull::from(&self.wait_list), None);
                    }
                    stack_init!(wait_entry = WaitEntry::insert_new(&self.wait_list));
                    while self.locked.get() {
                        debugln!("{tname}: [Mutex::lock] releasing spinlock and parking...");
                        drop(sguard);
                        park();
                        debugln!("{tname}: [Mutex::lock] unparked, getting spinlock...");
                        sguard = self.spin_lock.acquire();
                        debugln!("{tname}: [Mutex::lock] acquired spinlock");
                    }
                    debugln!("{tname}: [Mutex::lock] Mutex is now available, freeing wait list");
                    drop(wait_entry);
                    debug!("{tname}: [Mutex::lock] list status: ");
                    unsafe {
                        ListHead::debug_print(NonNull::from(&self.wait_list), None);
                    }
                },
            );
        }
        debugln!("{tname}: [Mutex::lock] locking mutex, returning guard and unlocking spinlock");
        self.locked.set(true);
        MutexGuard { mtx: self }
    }
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T> {
    mtx: &'a Mutex<T>,
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        let t = current();
        #[allow(unused)]
        let tname = t.name().unwrap_or("unnamed thread");
        debugln!("{tname}: [MutexGuard::drop] getting spinlock...");
        let sguard = self.mtx.spin_lock.acquire();
        debugln!("{tname}: [MutexGuard::drop] acquired spinlock");
        debug!("{tname}: [MutexGuard::drop] list status: ");
        unsafe {
            ListHead::debug_print(NonNull::from(&self.mtx.wait_list), None);
        }
        self.mtx.locked.set(false);
        if let Some(list_field) = self.mtx.wait_list.next() {
            let wait_entry = list_field.as_ptr().cast::<WaitEntry>();
            debugln!("{tname}: [MutexGuard::drop] waking up waiting thread ({wait_entry:p})");
            unsafe { (*wait_entry).thread.unpark() };
        }
        debugln!("{tname}: [MutexGuard::drop] unlocking spinlock");
        drop(sguard);
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.mtx.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.mtx.data.get() }
    }
}

#[repr(C)]
struct WaitEntry {
    wait_list: ListHead,
    thread: Thread,
}

impl WaitEntry {
    fn insert_new(list: &ListHead) -> impl PinInitializer<Self, !> + '_ {
        pin_init!(Self {
            thread: current(),
            wait_list: ListHead::insert_new(list),
        })
    }
}

fn main() {
    let mtx: Pin<Arc<Mutex<usize>>> = Arc::pin_init(Mutex::new(0)).unwrap();
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
