//! #![feature(generic_associated_types, const_ptr_offset_from, const_refs_to_cell)]
//! use core::{
//!     marker::PhantomPinned,
//!     mem,
//!     pin::Pin,
//!     ptr::{self, NonNull},
//! };
//! use pinned_init::prelude::*;
//!
//! #[manual_init(PinnedDrop)]
//! pub struct LinkedList<T> {
//!     #[init]
//!     prev: StaticUninit<Link<T>>,
//!     #[init]
//!     next: StaticUninit<Link<T>>,
//!     value: Option<T>,
//!     _pin: PhantomPinned,
//! }
//!
//! type Link<T> = NonNull<LinkedList<T>>;
//!
//! impl<T> PinnedInit for LinkedListUninit<T> {
//!     type Initialized = LinkedList<T>;
//!
//!     fn init_raw(mut this: NeedsPinnedInit<Self>) {
//!         let link = unsafe {
//!             // SAFETY: the pointer from NeedsPinnedInit is valid and we do not use it until we are
//!             // initialized.
//!             Link::new_unchecked(this.as_ptr_mut() as *mut LinkedList<T>)
//!         };
//!         let LinkedListOngoingInit {
//!             prev,
//!             next,
//!             value: _,
//!             _pin,
//!         } = this.begin_init();
//!         prev.init(link);
//!         next.init(link);
//!     }
//! }
//!
//! impl<T> LinkedListUninit<T> {
//!     pub fn new(value: T) -> Self {
//!         Self {
//!             prev: StaticUninit::uninit(),
//!             next: StaticUninit::uninit(),
//!             value: Some(value),
//!             _pin: PhantomPinned,
//!         }
//!     }
//! }
//!
//! impl<T> LinkedList<T> {
//!     pub fn insert_after(self: Pin<&mut Self>, value: T) {
//!         let mut this = self.project();
//!         let mut new = Box::pin(LinkedList::new(value)).init();
//!         let next = &mut **this.next;
//!         let next = unsafe {
//!             // SAFETY: the pointer is valid and points to initialized data
//!             next.as_mut()
//!         };
//!         mem::swap(&mut new.prev, &mut next.prev);
//!         mem::swap(&mut new.next, &mut this.next);
//!         // leak the box, so the allocation stays in the list
//!         unsafe {
//!             // SAFETY: we never move the given list for as long as there exist pointers to it
//!             Box::leak(Pin::into_inner_unchecked(new));
//!         }
//!     }
//!
//!     pub fn insert_before(self: Pin<&mut Self>, value: T) {
//!         let mut this = self.project();
//!         let mut new = Box::pin(LinkedList::new(value)).init();
//!         let prev = &mut **this.prev;
//!         let prev = unsafe {
//!             // SAFETY: the pointer is valid and points to initialized data
//!             prev.as_mut()
//!         };
//!         mem::swap(&mut new.next, &mut prev.next);
//!         mem::swap(&mut new.prev, &mut this.prev);
//!         // leak the box, so the allocation stays in the list
//!         unsafe {
//!             // SAFETY: we never move the given list for as long as there exist pointers to it
//!             Box::leak(Pin::into_inner_unchecked(new));
//!         }
//!     }
//!
//!     pub fn iter(&self) -> LLIter<'_, T> {
//!         LLIter {
//!             cur: self,
//!             begin: None,
//!         }
//!     }
//!
//!     pub fn iter_mut(self: Pin<&mut Self>) -> LLIterMut<'_, T> {
//!         LLIterMut {
//!             cur: self,
//!             begin: None,
//!         }
//!     }
//!
//!     pub fn value_mut(self: Pin<&mut Self>) -> &mut T {
//!         let this = self.project();
//!         this.value.as_mut().unwrap()
//!     }
//!
//!     pub fn unlink(mut this: Pin<Box<Self>>) -> T {
//!         let this = this.as_mut().project();
//!         if this.next != this.prev {
//!             // we need to remove references to us before we get dropped.
//!             unsafe {
//!                 // SAFETY: the pointers are valid and point to initialized data
//!                 let next = (**this.next).as_mut();
//!                 let prev = (**this.prev).as_mut();
//!                 mem::swap(&mut next.prev, &mut *this.prev);
//!                 mem::swap(&mut prev.next, &mut *this.next);
//!                 assert!(this.prev == this.next);
//!             }
//!         }
//!         this.value.take().unwrap()
//!     }
//! }
//!
//! #[pin_project::pinned_drop]
//! impl<T, const INIT: bool> PinnedDrop for LinkedList<T, INIT> {
//!     fn drop(self: Pin<&mut Self>) {
//!         fn inner<T>(mut cur: Pin<&mut LinkedList<T>>) {
//!             while {
//!                 let cur = cur.as_ref().project_ref();
//!                 cur.next != cur.prev
//!             } {
//!                 // we need to remove references to cur before we remove it.
//!                 unsafe {
//!                     let this = cur.project();
//!                     // SAFETY: the pointers are valid and point to initialized data
//!                     let next = (**this.next).as_mut();
//!                     let prev = (**this.prev).as_mut();
//!                     mem::swap(&mut next.prev, &mut *this.prev);
//!                     mem::swap(&mut prev.next, &mut *this.next);
//!                     assert!(this.prev == this.next);
//!                     // set the next node, all nodes are pinned
//!                     cur = Pin::new_unchecked(next);
//!                 }
//!             }
//!         }
//!         if INIT {
//!             inner::<T>(unsafe {
//!                 // SAFETY: we are initialized
//!                 core::mem::transmute(self)
//!             });
//!         }
//!     }
//! }
//!
//! pub struct LLIter<'a, T> {
//!     cur: &'a LinkedList<T>,
//!     begin: Option<&'a LinkedList<T>>,
//! }
//!
//! impl<'a, T> Iterator for LLIter<'a, T> {
//!     type Item = &'a T;
//!
//!     fn next(&mut self) -> Option<&'a T> {
//!         if let Some(begin) = self.begin {
//!             if ptr::eq(begin, self.cur) {
//!                 None
//!             } else {
//!                 todo!()
//!             }
//!         } else {
//!             self.begin = Some(self.cur);
//!             let val = self.cur.value.as_ref().unwrap();
//!             unsafe {
//!                 // SAFETY: node pointers are always init
//!                 self.cur = (*self.cur.next).as_ref();
//!             }
//!             Some(val)
//!         }
//!     }
//! }
//!
//! pub struct LLIterMut<'a, T> {
//!     cur: Pin<&'a mut LinkedList<T>>,
//!     begin: Option<NonNull<LinkedList<T>>>,
//! }
//!
//! impl<'a, T> Iterator for LLIterMut<'a, T> {
//!     type Item = &'a mut T;
//!
//!     fn next(&mut self) -> Option<&'a mut T> {
//!         if let Some(begin) = self.begin {
//!             if ptr::eq(begin.as_ptr(), &mut *self.cur) {
//!                 None
//!             } else {
//!                 todo!()
//!             }
//!         } else {
//!             self.begin = NonNull::new(&mut *self.cur as *mut LinkedList<T>);
//!             let val: &'a mut T = unsafe {
//!                 // SAFETY: we only go in one direction and we have borrowed a LinkedList,
//!                 // so no modifications can take place until 'a ends
//!                 mem::transmute(self.cur.value.as_mut().unwrap())
//!             };
//!             unsafe {
//!                 // SAFETY: node pointers are always init and pinned
//!                 self.cur = Pin::new_unchecked((*self.cur.next).as_mut());
//!             }
//!             Some(val)
//!         }
//!     }
//! }
//!
//! fn main() {}
