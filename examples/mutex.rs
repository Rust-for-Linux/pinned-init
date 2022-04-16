#![feature(generic_associated_types, const_ptr_offset_from, const_refs_to_cell)]
#![deny(unsafe_op_in_unsafe_fn)]
use core::{cell::*, marker::*, mem, pin::*, sync::atomic::*};
use pinned_init::{prelude::*, static_uninit::StaticUninit, *};

#[manual_init]
pub struct ListHead {
    #[init]
    next: StaticUninit<*mut ListHead>,
    #[init]
    prev: StaticUninit<*mut ListHead>,
    _pin: PhantomPinned,
}

impl PinnedInit for ListHeadUninit {
    type Initialized = ListHead;

    fn init_raw(mut this: NeedsPinnedInit<Self>) {
        let ptr = unsafe {
            // SAFETY: we obtain a raw pointer to `self`, this pointer is only
            // ever used by this `ListHead` and we take care never to return it
            // or use it to move this struct.
            this.as_ptr_mut() as *mut ListHead
        };
        let ListHeadOngoingInit { next, prev, .. } = this.begin_init();
        next.init(ptr);
        prev.init(ptr);
    }
}

impl ListHeadUninit {
    pub fn new() -> Self {
        Self {
            next: StaticUninit::uninit(),
            prev: StaticUninit::uninit(),
            _pin: PhantomPinned,
        }
    }
}

#[pinned_init]
pub struct Mutex<T> {
    inner: AtomicBool,
    #[init]
    wait_list: ListHead,
    value: UnsafeCell<T>,
}

impl<T> MutexUninit<T> {
    pub fn new(data: T) -> Self {
        Self {
            inner: AtomicBool::new(false),
            wait_list: ListHead::new(),
            value: UnsafeCell::new(data),
        }
    }
}

impl<T> Mutex<T> {
    pub fn lock(&self) -> &mut T {
        unsafe { &mut *self.value.get() }
    }
}

fn main() {}
