#![feature(generic_associated_types, const_ptr_offset_from, const_refs_to_cell)]
#![deny(unsafe_op_in_unsafe_fn)]
use core::{
    fmt,
    marker::PhantomPinned,
    mem::{self, MaybeUninit},
    pin::Pin,
    ptr::{self, NonNull},
};
use pinned_init::prelude::*;

#[manual_init(pinned, pin_project(PinnedDrop))]
pub struct LinkedList<T> {
    #[init]
    prev: Link<T>,
    #[init]
    next: Link<T>,
    value: Option<T>,
    #[pin]
    _pin: PhantomPinned,
}

#[manual_init]
struct Link<T> {
    #[init]
    #[uninit = MaybeUninit::<NonNull::<LinkedList::<T>>>]
    ptr: NonNull<LinkedList<T>>,
}

impl<T> Init for LinkUninit<T> {
    type Initialized = Link<T>;
    type Param = *mut LinkedList<T>;

    fn init_raw(this: NeedsInit<Self>, param: Self::Param) {
        let LinkOngoingInit { ptr } = this.begin_init();
        ptr.init(NonNull::new(param).unwrap());
    }
}

impl<T> LinkUninit<T> {
    fn uninit() -> Self {
        Self {
            ptr: MaybeUninit::uninit(),
        }
    }
}

impl<T> Link<T> {
    /// # Safety
    ///
    /// only call follow_mut in one direction
    ///
    /// need to handle lifetime carefully
    unsafe fn follow_mut_long<'a>(&mut self) -> &'a mut LinkedList<T> {
        unsafe {
            // SAFETY: we were initialized and thus point to a valid LinkedList
            self.ptr.as_mut()
        }
    }

    /// # Safety
    ///
    /// only call follow_mut in one direction
    unsafe fn follow_mut(&mut self) -> &mut LinkedList<T> {
        unsafe {
            // SAFETY: we were initialized and thus point to a valid LinkedList
            self.ptr.as_mut()
        }
    }

    fn follow(&self) -> &LinkedList<T> {
        unsafe {
            // SAFETY: we were initialized and thus point to a valid LinkedList
            self.ptr.as_ref()
        }
    }

    unsafe fn from_raw(raw: *mut LinkedList<T>) -> Self {
        Self {
            ptr: unsafe { NonNull::new_unchecked(raw) },
        }
    }

    unsafe fn as_raw(&self) -> *mut LinkedList<T> {
        self.ptr.as_ptr()
    }
}

impl<T> fmt::Pointer for Link<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:p}", self.ptr)
    }
}

impl<T> PartialEq for Link<T> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.ptr.as_ptr(), other.ptr.as_ptr())
    }
}

impl<T> Eq for Link<T> {}

impl<T> PinnedInit for LinkedListUninit<T> {
    type Initialized = LinkedList<T>;
    type Param = ();

    fn init_raw(mut this: NeedsPinnedInit<Self>, _: Self::Param) {
        let link = unsafe {
            // SAFETY: the pointer from NeedsPinnedInit is valid and we do not use it until we are
            // initialized.
            this.as_ptr_mut() as *mut LinkedList<T>
        };
        let LinkedListOngoingInit {
            prev,
            next,
            value: _,
            _pin,
        } = this.begin_init();
        Init::init_raw(next, link);
        Init::init_raw(prev, link);
    }
}

impl<T> LinkedListUninit<T> {
    pub fn new(value: T) -> Pin<Box<Self>> {
        Box::pin(Self {
            prev: LinkUninit::uninit(),
            next: LinkUninit::uninit(),
            value: Some(value),
            _pin: PhantomPinned,
        })
    }
}

impl<T> LinkedList<T> {
    pub fn insert_after(self: Pin<&mut Self>, value: T) {
        let new = Box::into_raw(unsafe {
            // SAFETY: we never move the contents of the box
            Pin::into_inner_unchecked(LinkedListUninit::new(value).init())
        });
        let mut this = self.project();
        let next = unsafe {
            // SAFETY: we only go forwards
            this.next.follow_mut()
        };
        unsafe {
            // SAFETY: new comes from a box and so is valid
            (*new).prev = Link::from_raw(new);
            (*new).next = Link::from_raw(new);
            mem::swap(&mut (*new).prev, &mut next.prev);
            mem::swap(&mut (*new).next, &mut this.next);
        }
    }

    pub fn insert_before(self: Pin<&mut Self>, value: T) {
        let new = Box::into_raw(unsafe {
            // SAFETY: we never move the contents of the box
            Pin::into_inner_unchecked(LinkedListUninit::new(value).init())
        });
        let mut this = self.project();
        let prev = unsafe {
            // SAFETY: we only go backwards
            this.prev.follow_mut()
        };
        unsafe {
            // SAFETY: new comes from a box and so is valid
            (*new).prev = Link::from_raw(new);
            (*new).next = Link::from_raw(new);
            mem::swap(&mut (*new).next, &mut prev.next);
            mem::swap(&mut (*new).prev, &mut this.prev);
        }
    }

    pub fn iter(&self) -> LLIter<'_, T> {
        LLIter {
            cur: self,
            begin: None,
        }
    }

    pub fn iter_mut(self: Pin<&mut Self>) -> LLIterMut<'_, T> {
        LLIterMut {
            cur: self,
            begin: None,
        }
    }

    pub fn value_mut(self: Pin<&mut Self>) -> &mut T {
        let this = self.project();
        this.value.as_mut().unwrap()
    }

    pub fn unlink(mut this: Pin<Box<Self>>) -> (T, Option<Pin<Box<LinkedList<T>>>>) {
        let this = this.as_mut().project();
        if this.next != this.prev {
            // we need to remove references to us before we get dropped.
            // SAFETY: next only goes forwards
            let next = unsafe { this.next.follow_mut_long() };
            // SAFETY: prev only goes backwards
            let prev = unsafe { this.prev.follow_mut_long() };
            mem::swap(&mut next.prev, &mut *this.prev);
            mem::swap(&mut prev.next, &mut *this.next);
            assert!(this.prev == this.next);
            (
                this.value.take().unwrap(),
                Some(unsafe {
                    // SAFETY: we only allow creation of LinkedList within Box and we were the previous
                    // owning node (consuming the parameter of this function)
                    Pin::new_unchecked(Box::from_raw(next as *mut LinkedList<T>))
                }),
            )
        } else {
            (this.value.take().unwrap(), None)
        }
    }
}

#[pin_project::pinned_drop]
impl<T> PinnedDrop for LinkedList<T> {
    fn drop(self: Pin<&mut Self>) {
        if !ptr::eq(self.next.ptr.as_ptr(), &*self) {
            // we need to remove references to cur before we remove it.
            let this = self.project();
            // SAFETY: the pointers are valid and point to initialized data
            // SAFETY: next only goes forwards and we are in drop, so the lifetime will end
            // after this function
            let next = unsafe { this.next.as_raw() };
            // SAFETY: prev only goes backwards and we are in drop, so the lifetime will end
            // after this function
            let prev = unsafe { this.prev.follow_mut_long() };
            mem::swap(unsafe { &mut (*next).prev }, &mut *this.prev);
            mem::swap(&mut prev.next, &mut *this.next);
            assert!(this.prev == this.next, "{:p} != {:p}", this.prev, this.next);
            // SAFETY: all nodes are pinned
            let mut cur = unsafe { Pin::new_unchecked(Box::from_raw(next)) };
            while !ptr::eq(cur.next.ptr.as_ptr(), &*cur) {
                // we need to remove references to cur before we remove it.
                let this = cur.as_mut().project();
                // SAFETY: the pointers are valid and point to initialized data
                // SAFETY: next only goes forwards and we are in drop, so the lifetime will end
                // after this function
                let next = unsafe { this.next.as_raw() };
                // SAFETY: prev only goes backwards and we are in drop, so the lifetime will end
                // after this function
                let prev = unsafe { this.prev.follow_mut_long() };
                mem::swap(unsafe { &mut (*next).prev }, &mut *this.prev);
                mem::swap(&mut prev.next, &mut *this.next);
                assert!(this.prev == this.next, "{:p} != {:p}", this.prev, this.next);
                // SAFETY: all nodes are pinned
                cur = unsafe { Pin::new_unchecked(Box::from_raw(next)) };
            }
        }
    }
}

#[pin_project::pinned_drop]
impl<T> PinnedDrop for LinkedListUninit<T> {
    fn drop(self: Pin<&mut Self>) {
        // we have not been initialized, so we cant be part of a list.
    }
}

pub struct LLIter<'a, T> {
    cur: &'a LinkedList<T>,
    begin: Option<&'a LinkedList<T>>,
}

impl<'a, T> Iterator for LLIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if let Some(begin) = self.begin {
            if ptr::eq(begin, self.cur) {
                None
            } else {
                todo!()
            }
        } else {
            self.begin = Some(self.cur);
            let val = self.cur.value.as_ref().unwrap();
            self.cur = self.cur.next.follow();
            Some(val)
        }
    }
}

pub struct LLIterMut<'a, T> {
    cur: Pin<&'a mut LinkedList<T>>,
    begin: Option<NonNull<LinkedList<T>>>,
}

impl<'a, T> Iterator for LLIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T> {
        if let Some(begin) = self.begin {
            if ptr::eq(begin.as_ptr(), &*self.cur) {
                None
            } else {
                todo!()
            }
        } else {
            unsafe {
                // SAFETY: we do not move out of self.begin
                self.begin =
                    NonNull::new(self.cur.as_mut().get_unchecked_mut() as *mut LinkedList<T>);
            }
            let cur = self.cur.as_mut().project();
            let val: &'a mut T = unsafe {
                // SAFETY: we only go in one direction and we have borrowed a LinkedList,
                // so no modifications can take place until 'a ends
                mem::transmute(cur.value.as_mut().unwrap())
            };
            unsafe {
                // SAFETY: node pointers are always init and pinned
                self.cur = Pin::new_unchecked(cur.next.follow_mut_long());
            }
            Some(val)
        }
    }
}

fn main() {
    let mut list = LinkedListUninit::new(42).init();
    list.as_mut().insert_after(1337);
    list.as_mut().insert_after(23);
    list.as_mut().insert_before(-42);
    list.as_mut().insert_before(-428);
}
