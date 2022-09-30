#![feature(
    type_alias_impl_trait,
    never_type,
    stmt_expr_attributes,
    raw_ref_op,
    new_uninit
)]

use core::{
    cell::Cell,
    marker::PhantomPinned,
    ptr::{self, NonNull},
};

use simple_safe_init::*;

#[repr(C)]
#[derive(Debug)]
pub struct ListHead {
    next: Link,
    prev: Link,
    pin: PhantomPinned,
}

impl ListHead {
    #[inline]
    pub fn new() -> impl PinInit<Self, !> {
        pin_init!(&this in Self {
            next: unsafe { Link::new_unchecked(this) },
            prev: unsafe { Link::new_unchecked(this) },
            pin: PhantomPinned,
        })
    }

    #[inline]
    pub fn insert_next(list: &ListHead) -> impl PinInit<Self, !> + '_ {
        pin_init!(&this in Self {
            prev: list.next.prev().replace(unsafe { Link::new_unchecked(this)}),
            next: list.next.replace(unsafe { Link::new_unchecked(this)}),
            pin: PhantomPinned,
        })
    }

    #[inline]
    pub fn insert_prev(list: &ListHead) -> impl PinInit<Self, !> + '_ {
        pin_init!(&this in Self {
            next: list.prev.next().replace(unsafe { Link::new_unchecked(this)}),
            prev: list.prev.replace(unsafe { Link::new_unchecked(this)}),
            pin: PhantomPinned,
        })
    }

    #[inline]
    pub fn next(&self) -> Option<NonNull<Self>> {
        if ptr::eq(self.next.as_ptr(), self) {
            None
        } else {
            Some(unsafe { NonNull::new_unchecked(self.next.as_ptr() as *mut Self) })
        }
    }
}

impl Drop for ListHead {
    #[inline]
    fn drop(&mut self) {
        if !ptr::eq(self.next.as_ptr(), self) {
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
        unsafe { Self(Cell::new(ptr)) }
    }

    #[inline]
    fn next(&self) -> &Link {
        unsafe { &(*self.0.get().as_ptr()).next }
    }

    #[inline]
    fn prev(&self) -> &Link {
        unsafe { &(*self.0.get().as_ptr()).prev }
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

#[allow(dead_code)]
fn main() -> Result<(), AllocInitErr<!>> {
    let a = Box::pin_init(ListHead::new())?;
    stack_init!(let b = ListHead::insert_next(&*a));
    let b = b?;
    stack_init!(let c = ListHead::insert_next(&*a));
    let c = c?;
    stack_init!(let d = ListHead::insert_next(&*b));
    let d = d?;
    let e = Box::pin_init(ListHead::insert_next(&*b))?;
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
