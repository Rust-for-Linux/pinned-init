Library to safely and fallibly initialize pinned structs in-place.

It also allows in-place initialization of big structs that would otherwise produce a stack overflow.

# The problem 

When writing self referential data structures in Rust, one runs into the issue
of initializing them. For example we will create an intrusive, doubly linked,
circular list in Rust:

```rust
pub struct ListHead {
    next: NonNull<Self>,
    prev: NonNull<Self>,
    // ListHead is `!Unpin` because `next.prev = self`
    _pin: PhantomPinned,
}
```

But now, how would one go about creating a `ListHead`? A valid initial state of
a singular ListHead is, with `next` and `prev` pointing to `self`. But in Rust
we cannot get a hold of `self` until we have selected a value for `next`!

## An unsafe solution

"Just do it like C", yes we can do just that:

```rust
impl ListHead {
    pub unsafe fn init(this: *mut Self) {
        addr_of_mut!((*this).next).write(this);
        addr_of_mut!((*this).prev).write(this);
    }
}

let list_head = MaybeUninit::uninit();
// SAFETY: list_head has a valid address on the stack.
unsafe { ListHead::init(list_head.as_mut_ptr()) };
// SAFETY: list_head was initialized above and is shadowed and will stay pinned on the stack
let list_head = unsafe { Pin::new_unchecked(list_head.assume_init_mut()) };
```

However this design is *very* bad:
- it is contagious: everyone containing a `ListHead` now needs to have an `unsafe fn init`
  to enable arbitrary construction and everyone using a `ListHead` will have to
  use `unsafe`.
- it is `unsafe`: easy to forget calling `init` at the right time.
- it is verbose: I have to 'manually' allocate uninitialized memory and handle
  the pinning myself!

In an ideal world, `ListHead` would wrap all of the unsafety that is needed for
making these kinds of lists work. It should only provide a safe interface and
easy initialization!

## Making a safe initialization API

Now lets try making a safe API. We will create a function that returns an
*initializer*:
```rust
impl ListHead {
    pub fn new() -> impl PinInitializer<Self, !> {
        todo!()
    }
}
```
We used `PinInitializer`, because our type requires a stable address after being
initialized. The error type (specified as the second parameter) is `!`, because
it can never fail to initialize a `ListHead`. If we had to allocate some
additional memory for our ListHead, we could change this to
`alloc::alloc::AllocError`.

But *how* do we create such an initializer, this library provides a macro for
this purpose:
```rust
impl ListHead {
    pub fn new() -> impl PinInitializer<Self, !> {
        // we specify `&this <-` to use it in the initializer
        pin_init!(&this <- Self {
            next: NonNull::from(this),
            prev: NonNull::from(this),
            _pin: PhantomPinned,
        });
    }
}
```

Great, now the user can get an initializer to `ListHead`, but how do they use
it? Well it depends on where they want it to be located. They might want it
inside of a `Box`, an `Arc` or maybe on the stack. That is why this library
provides an extension trait for the smart pointers and a macro to use an
initializer on the stack:
```rust
stack_init!(list_head = ListHead::new());

let _: Result<Pin<Box<ListHead>>, InitAllocErr<!>> = Box::pin_init(ListHead::new());
let _: Result<Pin<Arc<ListHead>>, InitAllocErr<!>> = Arc::pin_init(ListHead::new());
let _: Result<Pin<Rc<ListHead>>, InitAllocErr<!>> = Rc::pin_init(ListHead::new());
```

The `InitAllocErr<!>` is an error enum that either holds an initialization error
(`!` in this case) or an allocation error (creating a `Box` might fail after all).

### Behind the macro magic

The macro expands to this:
```rust
impl ListHead {
    pub fn new() -> impl PinInitializer<Self, !> {
        let init = move |place: *mut Self| -> ::core::result::Result<(), _> {
            let this = unsafe { ::core::ptr::NonNull::new_unchecked(place) };
            let next = this;
            unsafe {
                ::simple_safe_init::PinInitializer::__init_pinned(next, &raw mut (*place).next)?
            };
            let next = unsafe { ::simple_safe_init::DropGuard::new(&raw mut (*place).next) };
            let prev = this;
            unsafe {
                ::simple_safe_init::PinInitializer::__init_pinned(prev, &raw mut (*place).prev)?
            };
            let prev = unsafe { ::simple_safe_init::DropGuard::new(&raw mut (*place).prev) };
            let pin = PhantomPinned;
            unsafe {
                ::simple_safe_init::PinInitializer::__init_pinned(pin, &raw mut (*place).pin)?
            };
            let pin = unsafe { ::simple_safe_init::DropGuard::new(&raw mut (*place).pin) };
            #[allow(unreachable_code, clippy::diverging_sub_expression)]
            if false {
                let _: Self = Self {
                    next: ::core::panicking::panic("not yet implemented"),
                    prev: ::core::panicking::panic("not yet implemented"),
                    pin: ::core::panicking::panic("not yet implemented"),
                };
            }
            ::core::mem::forget(next);
            ::core::mem::forget(prev);
            ::core::mem::forget(pin);
            Ok(())
        };
        let init = unsafe { ::simple_safe_init::PinInit::from_closure(init) };
        init
    }
}
```
Lets unpack that a bit.

At the very beginning, the macro defines a closure:
```rust
let init = move |place: *mut Self| -> ::core::result::Result<(), _> {
```
`place` is a trusted parameter that came from unsafe code. It points to valid,
but uninitialized memory.

Next the macro creates a `NonNull` pointer from `place`, because we requested
it to do so by prepending `&this <-` to the invocation.

Every field is initialized in the same way. Let take `next` as an example:
```rust
// first we assign a variable with the specified field value, in our case it is `this`.
let next = this;
// then we use unsafe to initialize the field that is being offset with the value from above
// also note the `?` at the end to signify possible failure
unsafe {
    ::simple_safe_init::PinInitializer::__init_pinned(next, &raw mut (*place).next)?
};
// then we create a drop guard to drop the already initialized value. this is
// explained in detail below. It will drop the contents when an error/panic occurs.
let next = unsafe { ::simple_safe_init::DropGuard::new(&raw mut (*place).next) };
```
After every field has been initialized in the given order, the macro emits
```rust
#[allow(unreachable_code, clippy::diverging_sub_expression)]
if false {
    let _: Self = Self {
        next: ::core::panicking::panic("not yet implemented"),
        prev: ::core::panicking::panic("not yet implemented"),
        pin: ::core::panicking::panic("not yet implemented"),
    };
}
```
This code is not executed, but still type checked. It ensures that every field
was initialized and that no field was initialized multiple times. If this is not
the case, the macro would be unsound, because it allows `unsafe` code to call
`MaybeUninit::assume_init` when it returns `Ok`. But because of this the program
would fail to compile and thus the macro is sound.

After that all of the drop guards need to be `forget`ten, otherwise we would
drop the just initialized values.

At the very end, the closure is wrapped in a special type that is only
constructible via `unsafe` ensuring that the closure follows the following
invariants:
- after successful completion (closure returned `Ok`) the pointee of `place` has
  been fully initialized.
- after unsuccessful compilation (closure returned `Err` or `panic`ed) the pointee
  of `place` is fully uninitialized. This means that partially initialized values
  have been `drop`ped and it is safe to deallocate the memory.

All code emitted by the macros of this library abide by these invariants.

### Ensuring `PinInitializer` cannot be used to initialize in unpinned memory

This is done by making `Initializer` a sub-trait of `PinInitializer`. So one can
use any `Initializer` (or `PinInitializer`) to call `Box::pin_init`. But calling
`Box::init` can only be done with `Initializer`s, not `PinInitializer`s. It is
up to smart pointer authors to ensure this.

# Syntax experimentation

scottmcm on [zulip](https://rust-lang.zulipchat.com/#narrow/stream/213817-t-lang/topic/safe.20initialization/near/298893680) gave me the idea to use `<-` as a possible token for initialization.
So here is some experimental syntax. I am not sure if it is a good idea to make
initializers behave like normal values (as is seen in my macros).
One could use `<-` to make them more explicit:

```rust
pub struct ListHead {
    // using *const for convenicen
    next: *const Self,
    prev: *const Self,
    _pin: PhantomPinned,
}

impl ListHead {
    // note the reversed arrow! (maybe use `Self <-` instead, but that might be
    // more difficult for the parser)
    pub fn new() <- Self {
        <- Self {
            // self is available in the initializer context
            next: &raw const self,
            prev: &raw const self,
            _pin: PhantomPinned,
        }
    }
}

let list_head <- ListHead::new();
```
`list_head` would now be of the type `Pin<&mut ListHead>` and the storage would
be inaccessible.

```rust
pub struct Mutex<T> {
    wait_list: ListHead,
    value: UnsafeCell<T>,
}

impl Mutex<T> {
    // here the alternative way to write `<- Self` in the signature:
    pub fn new(data: T) Self <- {
        <- Self {
            wait_list <- ListHead::new(),
            value: UnsafeCell::new(data)
        }
    }
}
```
