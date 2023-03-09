#![feature(allocator_api)]
use core::{
    alloc::AllocError,
    cell::Cell,
    convert::Infallible,
    marker::PhantomPinned,
    pin::Pin,
    ptr::{self, NonNull},
};

use pinned_init::*;

#[pin_data(PinnedDrop)]
#[repr(C)]
#[derive(Debug)]
pub struct ListHead {
    next: Link,
    prev: Link,
    #[pin]
    pin: PhantomPinned,
}

impl ListHead {
    #[inline]
    pub fn new() -> impl PinInit<Self, Infallible> {
        try_pin_init!(&this in Self {
            next: unsafe { Link::new_unchecked(this) },
            prev: unsafe { Link::new_unchecked(this) },
            pin: PhantomPinned,
        }? Infallible)
    }

    #[inline]
    pub fn insert_next(list: &ListHead) -> impl PinInit<Self, Infallible> + '_ {
        try_pin_init!(&this in Self {
            prev: list.next.prev().replace(unsafe { Link::new_unchecked(this)}),
            next: list.next.replace(unsafe { Link::new_unchecked(this)}),
            pin: PhantomPinned,
        }? Infallible)
    }

    #[inline]
    pub fn insert_prev(list: &ListHead) -> impl PinInit<Self, Infallible> + '_ {
        try_pin_init!(&this in Self {
            next: list.prev.next().replace(unsafe { Link::new_unchecked(this)}),
            prev: list.prev.replace(unsafe { Link::new_unchecked(this)}),
            pin: PhantomPinned,
        }? Infallible)
    }

    #[inline]
    pub fn next(&self) -> Option<NonNull<Self>> {
        if ptr::eq(self.next.as_ptr(), self) {
            None
        } else {
            Some(unsafe { NonNull::new_unchecked(self.next.as_ptr() as *mut Self) })
        }
    }

    pub fn size(&self) -> usize {
        let mut size = 1;
        let mut cur = self.next.clone();
        while !ptr::eq(self, cur.cur()) {
            cur = cur.next().clone();
            size += 1;
        }
        size
    }
}

#[pinned_drop]
impl PinnedDrop for ListHead {
    //#[inline]
    fn drop(self: Pin<&mut Self>) {
        if !ptr::eq(self.next.as_ptr(), &*self) {
            let next = unsafe { &*self.next.as_ptr() };
            let prev = unsafe { &*self.prev.as_ptr() };
            next.prev.set(&self.prev);
            prev.next.set(&self.next);
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Debug)]
struct Link(Cell<NonNull<ListHead>>);

impl Link {
    #[inline]
    unsafe fn new_unchecked(ptr: NonNull<ListHead>) -> Self {
        Self(Cell::new(ptr))
    }

    #[inline]
    fn next(&self) -> &Link {
        unsafe { &(*self.0.get().as_ptr()).next }
    }

    #[inline]
    fn prev(&self) -> &Link {
        unsafe { &(*self.0.get().as_ptr()).prev }
    }

    fn cur(&self) -> &ListHead {
        unsafe { &*self.0.get().as_ptr() }
    }

    #[inline]
    fn replace(&self, other: Link) -> Link {
        unsafe { Link::new_unchecked(self.0.replace(other.0.get())) }
    }

    #[inline]
    fn as_ptr(&self) -> *const ListHead {
        self.0.get().as_ptr()
    }

    #[inline]
    fn set(&self, val: &Link) {
        self.0.set(val.0.get());
    }
}

macro_rules! into_ok {
    ($v:expr) => {
        match $v {
            Ok(v) => v,
            Err(i) => match i {},
        }
    };
}

#[allow(dead_code)]
fn main() -> Result<(), AllocError> {
    let a = Box::pin_init(ListHead::new())?;
    stack_pin_init!(let b = ListHead::insert_next(&a));
    let b = into_ok!(b);
    stack_pin_init!(let c = ListHead::insert_next(&a));
    let c = into_ok!(c);
    stack_pin_init!(let d = ListHead::insert_next(&b));
    let d = into_ok!(d);
    let e = Box::pin_init(ListHead::insert_next(&b))?;
    println!("a ({a:p}): {a:?}");
    println!("b ({b:p}): {b:?}");
    println!("c ({c:p}): {c:?}");
    println!("d ({d:p}): {d:?}");
    println!("e ({e:p}): {e:?}");
    let mut inspect = &*a;
    while let Some(next) = inspect.next() {
        println!("({inspect:p}): {inspect:?}");
        inspect = unsafe { &*next.as_ptr() };
        if core::ptr::eq(inspect, &*a) {
            break;
        }
    }
    Ok(())
}
