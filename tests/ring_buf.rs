#![allow(clippy::undocumented_unsafe_blocks)]
#![cfg_attr(feature = "alloc", feature(allocator_api))]

use core::{
    convert::Infallible,
    marker::PhantomPinned,
    mem::MaybeUninit,
    pin::Pin,
    ptr::{self, addr_of_mut},
};
use pinned_init::*;
use std::sync::Arc;

#[expect(unused_attributes)]
#[path = "../examples/mutex.rs"]
mod mutex;
use mutex::*;

#[expect(unused_attributes)]
#[path = "../examples/error.rs"]
mod error;
use error::Error;

#[pin_data(PinnedDrop)]
pub struct RingBuffer<T, const SIZE: usize> {
    buffer: [MaybeUninit<T>; SIZE],
    head: *mut T,
    tail: *mut T,
    #[pin]
    _pin: PhantomPinned,
}

#[pinned_drop]
impl<T, const SIZE: usize> PinnedDrop for RingBuffer<T, SIZE> {
    fn drop(self: Pin<&mut Self>) {
        // SAFETY: We do not move `this`.
        let this = unsafe { self.get_unchecked_mut() };
        while !ptr::eq(this.tail, this.head) {
            unsafe { this.tail.drop_in_place() };
            this.tail = unsafe { this.advance(this.tail) };
        }
    }
}

impl<T, const SIZE: usize> RingBuffer<T, SIZE> {
    pub fn new() -> impl PinInit<Self> {
        assert!(SIZE > 0);
        pin_init!(&this in Self {
            // SAFETY: The elements of the array can be uninitialized.
            buffer <- unsafe { init_from_closure(|_| Ok::<_, Infallible>(())) },
            // SAFETY: `this` is a valid pointer.
            head: unsafe { addr_of_mut!((*this.as_ptr()).buffer).cast::<T>() },
            tail: unsafe { addr_of_mut!((*this.as_ptr()).buffer).cast::<T>() },
            _pin: PhantomPinned,
        })
    }

    pub fn push(self: Pin<&mut Self>, value: impl Init<T>) -> bool {
        match self.try_push(value) {
            Ok(res) => res,
            Err(i) => match i {},
        }
    }
    pub fn try_push<E>(self: Pin<&mut Self>, value: impl Init<T, E>) -> Result<bool, E> {
        // SAFETY: We do not move `this`.
        let this = unsafe { self.get_unchecked_mut() };
        let next_head = unsafe { this.advance(this.head) };
        // `head` and `tail` point into the same buffer.
        if ptr::eq(next_head, this.tail) {
            // We cannot advance `head`, since `next_head` would point to the same slot as `tail`,
            // which is currently live.
            return Ok(false);
        }
        // SAFETY: `head` always points to the next free slot.
        unsafe { value.__init(this.head)? };
        this.head = next_head;
        Ok(true)
    }

    pub fn pop(self: Pin<&mut Self>) -> Option<T> {
        // SAFETY: We do not move `this`.
        let this = unsafe { self.get_unchecked_mut() };
        if ptr::eq(this.head, this.tail) {
            return None;
        }
        // SAFETY: `tail` always points to a valid element, or is the same as `head`.
        let value = unsafe { this.tail.read() };
        this.tail = unsafe { this.advance(this.tail) };
        Some(value)
    }

    pub fn pop_no_stack(self: Pin<&mut Self>) -> Option<impl Init<T> + '_> {
        // SAFETY: We do not move `this`.
        let this = unsafe { self.get_unchecked_mut() };
        if ptr::eq(this.head, this.tail) {
            return None;
        }
        let remove_init = |slot| {
            // SAFETY: `tail` always points to a valid element, or is the same as `head`.
            unsafe { ptr::copy_nonoverlapping(this.tail, slot, 1) };
            this.tail = unsafe { this.advance(this.tail) };
            Ok(())
        };
        // SAFETY: the above initializer is correct.
        Some(unsafe { init_from_closure(remove_init) })
    }

    /// # Safety
    ///
    /// TODO
    unsafe fn advance(&mut self, ptr: *mut T) -> *mut T {
        // SAFETY: ptr's offset from buffer is < SIZE
        let ptr = unsafe { ptr.add(1) };
        let origin: *mut _ = addr_of_mut!(self.buffer);
        let origin = origin.cast::<T>();
        let offset = unsafe { ptr.offset_from(origin) };
        if offset >= SIZE as isize {
            origin
        } else {
            ptr
        }
    }
}

#[test]
fn on_stack() -> Result<(), Infallible> {
    stack_pin_init!(let buf = RingBuffer::<u8, 64>::new());
    if let Some(elem) = buf.as_mut().pop() {
        panic!("found in empty buffer!: {elem}");
    }
    assert!(buf.as_mut().push(10));
    assert!(buf.as_mut().push(42));
    assert_eq!(buf.as_mut().pop(), Some(10));
    assert_eq!(buf.as_mut().pop(), Some(42));
    assert_eq!(buf.as_mut().pop(), None);
    assert!(buf.as_mut().push(42));
    assert!(buf.as_mut().push(24));
    assert_eq!(buf.as_mut().pop(), Some(42));
    assert!(buf.as_mut().push(25));
    assert_eq!(buf.as_mut().pop(), Some(24));
    assert_eq!(buf.as_mut().pop(), Some(25));
    assert_eq!(buf.as_mut().pop(), None);
    for i in 0..63 {
        assert!(buf.as_mut().push(i));
    }
    assert!(!buf.as_mut().push(42));
    for i in 0..63 {
        if let Some(value) = buf.as_mut().pop_no_stack() {
            stack_pin_init!(let value = value);
            assert_eq!(*value, i);
        } else {
            panic!("Expected more values");
        }
    }
    assert_eq!(buf.as_mut().pop(), None);
    Ok(())
}

#[derive(PartialEq, Eq, Debug)]
pub struct EvenU64 {
    info: String,
    data: u64,
}

impl EvenU64 {
    pub fn new2(value: u64) -> impl Init<Self, Error> {
        try_init!(Self {
            info: "Hello world!".to_owned(),
            data: if value % 2 == 0 {
                value
            } else {
                return Err(Error);
            },
        }? Error)
    }
    pub fn new(value: u64) -> impl Init<Self, ()> {
        try_init!(Self {
            info: "Hello world!".to_owned(),
            data: if value % 2 == 0 {
                value
            } else {
                return Err(());
            },
        }?())
    }
}

#[test]
fn even_stack() {
    stack_try_pin_init!(let val = EvenU64::new(0));
    assert_eq!(
        val.as_deref_mut(),
        Ok(&mut EvenU64 {
            info: "Hello world!".to_owned(),
            data: 0
        })
    );
    stack_try_pin_init!(let val = EvenU64::new(1));
    assert_eq!(val, Err(()));
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[test]
fn even_failing() {
    assert!(matches!(Box::try_pin_init(EvenU64::new2(3)), Err(Error)));
    assert!(matches!(Box::try_init(EvenU64::new2(3)), Err(Error)));
    assert!(matches!(Arc::try_pin_init(EvenU64::new2(5)), Err(Error)));
    assert!(matches!(Box::try_init(EvenU64::new2(3)), Err(Error)));
    assert!(matches!(Arc::try_init(EvenU64::new2(5)), Err(Error)));
}

#[test]
fn with_failing_inner() {
    let mut buf = Box::pin_init(RingBuffer::<EvenU64, 4>::new()).unwrap();
    assert_eq!(buf.as_mut().try_push(EvenU64::new(0)), Ok(true));
    assert_eq!(buf.as_mut().try_push(EvenU64::new(1)), Err(()));
    assert_eq!(buf.as_mut().try_push(EvenU64::new(2)), Ok(true));
    assert_eq!(buf.as_mut().try_push(EvenU64::new(3)), Err(()));
    assert_eq!(buf.as_mut().try_push(EvenU64::new(4)), Ok(true));
    assert_eq!(buf.as_mut().try_push(EvenU64::new(5)), Ok(false));
    assert_eq!(buf.as_mut().try_push(EvenU64::new(6)), Ok(false));

    assert_eq!(
        buf.as_mut().pop(),
        Some(EvenU64 {
            info: "Hello world!".to_owned(),
            data: 0
        })
    );
    assert_eq!(
        buf.as_mut().pop(),
        Some(EvenU64 {
            info: "Hello world!".to_owned(),
            data: 2
        })
    );
    assert_eq!(
        buf.as_mut().pop(),
        Some(EvenU64 {
            info: "Hello world!".to_owned(),
            data: 4
        })
    );
    assert_eq!(buf.as_mut().pop(), None);
}

#[derive(Debug)]
struct BigStruct {
    buf: [u8; 1024 * 1024],
    oth: MaybeUninit<u8>,
}

#[cfg(all(any(feature = "std", feature = "alloc"), not(miri)))]
#[test]
fn big_struct() {
    let x = Arc::init(init!(BigStruct {
        buf <- zeroed(),
        oth <- zeroed(),
    }));
    println!("{x:?}");
    let x = Box::init(init!(BigStruct {
        buf <- zeroed(),
        oth <- zeroed(),
    }));
    println!("{x:?}");
}

#[cfg(all(any(feature = "std", feature = "alloc"), not(miri)))]
#[test]
fn with_big_struct() {
    let buf = Arc::pin_init(CMutex::new(RingBuffer::<BigStruct, 64>::new())).unwrap();
    let mut buf = buf.lock();
    for _ in 0..63 {
        assert_eq!(
            buf.as_mut().try_push(init!(BigStruct{
                buf <- zeroed(),
                oth <- uninit::<_, Infallible>(),
            })),
            Ok(true)
        );
    }
    assert_eq!(
        buf.as_mut().try_push(init!(BigStruct{
            buf <- zeroed(),
            oth <- uninit::<_, Infallible>(),
        })),
        Ok(false)
    );
    for _ in 0..63 {
        assert!(buf.as_mut().pop_no_stack().is_some());
    }
}
