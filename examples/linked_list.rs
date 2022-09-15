#![feature(
    type_alias_impl_trait,
    never_type,
    stmt_expr_attributes,
    raw_ref_op,
    new_uninit
)]

use core::{
    cell::{Cell, UnsafeCell},
    marker::PhantomPinned,
    ptr::{self, NonNull},
};

use simple_safe_init::*;

#[repr(C)]
pub struct ListHead {
    next: Link,
    prev: Link,
    pin: PhantomPinned,
}

impl ListHead {
    pub fn new() -> impl PinInitializer<Self, !> {
        pin_init!(&this <- Self {
            next: unsafe { Link::new_unchecked(this.as_ptr()) },
            prev: unsafe { Link::new_unchecked(this.as_ptr()) },
            pin: PhantomPinned,
        })
    }

    pub fn insert_new(list: &ListHead) -> impl PinInitializer<Self, !> + '_ {
        pin_init!(&this <- Self {
            prev: unsafe { Link::replace_raw(list.next.prev(),  Link::new_unchecked(this.as_ptr()) )},
            next: list.next.replace(unsafe { Link::new_unchecked(this.as_ptr()) }),
            pin: PhantomPinned,
        })
    }

    pub fn next(&self) -> Option<NonNull<Self>> {
        if ptr::eq(self.next.as_ptr(), self) {
            None
        } else {
            Some(unsafe { NonNull::new_unchecked(self.next.as_ptr() as *mut Self) })
        }
    }

    pub unsafe fn debug_print(this: NonNull<Self>, mut until: Option<NonNull<Self>>) {
        if false {
            if until.map(|t| this == t).unwrap_or(false) {
                println!("({:p}) }}", this);
                return;
            }
            if until.get_or_insert(this) == &this {
                print!("{{ ");
            }
            print!("{:p} -> ", this);
            Self::debug_print(
                *(&raw const (*this.as_ptr()).next).cast::<NonNull<Self>>(),
                until,
            );
        }
    }
}

impl Drop for ListHead {
    fn drop(&mut self) {
        //println!("dropping {self:p}...");
        if let Some(next) = self.next() {
            let next = next.as_ptr();
            let prev = self.prev.as_ptr() as *mut Self;
            unsafe {
                Link::set_raw(&raw mut (*next).prev, prev);
                Link::set_raw(&raw mut (*prev).next, next);
            }
        }
    }
}

#[repr(transparent)]
#[derive(Clone)]
struct Link(Cell<NonNull<ListHead>>);

impl Link {
    unsafe fn new_unchecked(ptr: *const ListHead) -> Self {
        unsafe { Self(Cell::new(NonNull::new_unchecked(ptr as *mut ListHead))) }
    }

    fn prev(&self) -> *const Link {
        unsafe { &raw const (*self.0.get().as_ptr()).prev }
    }

    fn replace(&self, other: Link) -> Link {
        unsafe { Link::new_unchecked(self.0.replace(other.0.get()).as_ptr()) }
    }

    unsafe fn replace_raw(this: *const Self, other: Link) -> Link {
        let loc: *mut *mut ListHead =
            UnsafeCell::raw_get(this.cast::<UnsafeCell<NonNull<ListHead>>>())
                .cast::<*mut ListHead>();
        let val: *mut ListHead = other.0.into_inner().as_ptr();
        unsafe { Link::new_unchecked(ptr::replace::<*mut ListHead>(loc, val)) }
    }

    fn as_ptr(&self) -> *const ListHead {
        self.0.get().as_ptr()
    }

    unsafe fn set_raw(this: *mut Self, val: *mut ListHead) {
        let loc: *mut *mut ListHead =
            UnsafeCell::raw_get(this.cast::<UnsafeCell<NonNull<ListHead>>>())
                .cast::<*mut ListHead>();
        loc.write(val);
    }
}

#[allow(dead_code)]
fn main() {}
