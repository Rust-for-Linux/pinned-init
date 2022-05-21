#![feature(generic_associated_types, const_ptr_offset_from, const_refs_to_cell)]
#![deny(unsafe_op_in_unsafe_fn)]
use core::{
    cell::UnsafeCell,
    marker::PhantomPinned,
    mem::{self, ManuallyDrop, MaybeUninit},
    pin::Pin,
    ptr::addr_of,
};
use pinned_init::prelude::*;

#[repr(transparent)]
struct UnsafeAliasCell<T> {
    value: UnsafeCell<T>,
    _pin: PhantomPinned,
}

impl<T> UnsafeAliasCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            _pin: PhantomPinned,
        }
    }

    pub fn get(&self) -> *mut T {
        self.value.get()
    }
}

#[manual_init(pinned, pin_project(PinnedDrop))]
pub struct PtrBuf<T, const N: usize> {
    #[init]
    #[uninit = MaybeUninit::<*const T>]
    idx: *const T,
    #[init]
    #[uninit = MaybeUninit::<*const T>]
    end: *const T,
    #[pin]
    buf: UnsafeAliasCell<[ManuallyDrop<T>; N]>,
}

impl<T, const N: usize> PinnedInit for PtrBufUninit<T, N> {
    type Initialized = PtrBuf<T, N>;
    type Param = ();

    fn init_raw(this: NeedsPinnedInit<Self>, _: Self::Param) {
        let PtrBufOngoingInit { idx, end, buf } = this.begin_init();
        let ptr = unsafe { addr_of!(*buf.get()) as *const T };
        idx.init(ptr);
        let ptr = unsafe { addr_of!((*buf.get())[N - 1]) as *const ManuallyDrop<T> as *const T };
        end.init(ptr);
    }
}

impl<T, const N: usize> From<[T; N]> for PtrBufUninit<T, N> {
    fn from(arr: [T; N]) -> Self {
        assert_ne!(N, 0);
        assert!(
            N < isize::MAX as usize,
            "N cannot be bigger than isize::MAX"
        );
        Self {
            idx: MaybeUninit::uninit(),
            end: MaybeUninit::uninit(),
            buf: unsafe {
                // SAFETY: T and ManuallyDrop<T> have the same layout, so [T; N] and
                // [ManuallyDrop<T>; N] also have the same layout
                let ptr = addr_of!(arr) as *const [T; N] as *const [ManuallyDrop<T>; N];
                mem::forget(arr);
                UnsafeAliasCell::new(ptr.read())
            },
        }
    }
}

impl<T, const N: usize> PtrBuf<T, N> {
    pub fn next(self: Pin<&mut Self>) -> Option<T> {
        let this = self.project();
        if this.idx > this.end {
            None
        } else {
            let val = unsafe {
                // SAFETY: we checked if idx is in bounds before and
                // we read the value at idx and offset idx, this value is never read again
                this.idx.read()
            };
            unsafe {
                // SAFETY: we are still in bounds of buf (end stores the end of buf)
                // and adding size_of::<T>() will only land us at most one byte of buf.
                *this.idx = this.idx.offset(1);
            }
            Some(val)
        }
    }
}

#[pin_project::pinned_drop]
impl<T, const N: usize> PinnedDrop for PtrBuf<T, N> {
    fn drop(mut self: Pin<&mut Self>) {
        while let Some(x) = self.as_mut().next() {
            drop(x);
        }
    }
}

#[pin_project::pinned_drop]
impl<T, const N: usize> PinnedDrop for PtrBufUninit<T, N> {
    fn drop(self: Pin<&mut Self>) {
        let this = self.project();
        for x in unsafe { &mut *this.buf.get() } {
            unsafe {
                // SAFETY: we are in drop and only called once
                ManuallyDrop::drop(x)
            }
        }
    }
}

fn main() {
    let buf: PtrBufUninit<i32, 5> = [42, -42, 1337, 0, 6].into();
    let mut buf = Box::pin(buf).init();
    assert_eq!(buf.as_mut().next(), Some(42));
    assert_eq!(buf.as_mut().next(), Some(-42));
    assert_eq!(buf.as_mut().next(), Some(1337));
    assert_eq!(buf.as_mut().next(), Some(0));
    assert_eq!(buf.as_mut().next(), Some(6));
    assert_eq!(buf.as_mut().next(), None);
}
