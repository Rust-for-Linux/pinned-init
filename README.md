Library to safely and fallibly initialize pinned `struct`s using in-place constructors.

It also allows in-place initialization of big `struct`s that would otherwise produce a stack
overflow.

This library's main use-case is in [Rust-for-Linux]. Although this version can be used
standalone.

There are cases when you want to in-place initialize a struct. For example when it is very big
and moving it from the stack is not an option, because it is bigger than the stack itself.
Another reason would be that you need the address of the object to initialize it. This stands
in direct conflict with Rust's normal process of first initializing an object and then moving
it into it's final memory location.

This library allows you to do in-place initialization safely.

This library requires unstable features and thus can only be used with a nightly compiler.

# Overview

To initialize a `struct` with an in-place constructor you will need two things:
- an in-place constructor,
- a memory location that can hold your `struct` (this can be the [stack], an [`Arc<T>`],
  [`Box<T>`] or any other smart pointer that implements [`InPlaceInit`]).

To get an in-place constructor there are generally three options:
- directly creating an in-place constructor using the [`pin_init!`] macro,
- a custom function/macro returning an in-place constructor provided by someone else,
- using the unsafe function [`pin_init_from_closure()`] to manually create an initializer.

Aside from pinned initialization, this library also supports in-place construction without pinning,
the marcos/types/functions are generally named like the pinned variants without the `pin`
prefix.

# Examples

Throught some examples we will make use of the `CMutex` type which can be found in the examples
directory of the repository. It is essentially a rebuild of the `mutex` from the Linux kernel
in userland. So it also uses a wait list and a basic spinlock. Importantly it needs to be
pinned to be locked and thus is a prime candidate for this library.

## Using the [`pin_init!`] macro

If you want to use [`PinInit`], then you will have to annotate your `struct` with
`#[`[`pin_data`]`]`. It is a macro that uses `#[pin]` as a marker for
[structurally pinned fields]. After doing this, you can then create an in-place constructor via
[`pin_init!`]. The syntax is almost the same as normal `struct` initializers. The difference is
that you need to write `<-` instead of `:` for fields that you want to initialize in-place.

```rust
use pinned_init::*;
#[pin_data]
struct Foo {
    #[pin]
    a: CMutex<usize>,
    b: u32,
}

let foo = pin_init!(Foo {
    a <- CMutex::new(42),
    b: 24,
});
```

`foo` now is of the type [`impl PinInit<Foo>`]. We can now use any smart pointer that we like
(or just the stack) to actually initialize a `Foo`:

```rust
let foo: Result<Pin<Box<Foo>>, core::alloc::AllocError> = Box::pin_init(foo);
```

For more information see the [`pin_init!`] macro.

## Using a custom function/macro that returns an initializer

Many types that use this library supply a function/macro that returns an initializer, because
the above method only works for types where you can access the fields.

```rust
let mtx: Result<Pin<Box<CMutex<usize>>>, AllocError> = Box::pin_init(CMutex::new(42));
```

To declare an init macro/function you just return an [`impl PinInit<T, E>`]:

```rust
use core::alloc::AllocError;
#[pin_data]
struct DriverData {
    #[pin]
    status: CMutex<i32>,
    buffer: Box<[u8; 1_000_000]>,
}

struct DriverDataError;

impl DriverData {
    fn new() -> impl PinInit<Self, DriverDataError> {
        try_pin_init!(Self {
            status <- CMutex::new(0),
            buffer: Box::init(zeroed())?,
        }? DriverDataError)
    }
}
```

## Manual creation of an initializer

Often when working with primitives the previous approaches are not sufficient. That is where
[`pin_init_from_closure()`] comes in. This `unsafe` function allows you to create a
[`impl PinInit<T, E>`] directly from a closure. Of course you have to ensure that the closure
actually does the initialization in the correct way. Here are the things to look out for
(we are calling the parameter to the closure `slot`):
- when the closure returns `Ok(())`, then it has completed the initialization successfully, so
  `slot` now contains a valid bit pattern for the type `T`,
- when the closure returns `Err(e)`, then the caller may deallocate the memory at `slot`, so
  you need to take care to clean up anything if your initialization fails mid-way,
- you may assume that `slot` will stay pinned even after the closure returns until `drop` of
  `slot` gets called.

```rust
use pinned_init::*;
use core::{ptr::addr_of_mut, marker::PhantomPinned, cell::UnsafeCell};
mod bindings {
    extern "C" {
        pub type foo;
        pub fn init_foo(ptr: *mut foo);
        pub fn destroy_foo(ptr: *mut foo);
        #[must_use = "you must check the error return code"]
        pub fn enable_foo(ptr: *mut foo, flags: u32) -> i32;
    }
}
pub struct RawFoo {
    _p: PhantomPinned,
    foo: UnsafeCell<bindings::foo>,
}

impl RawFoo {
    pub fn new(flags: u32) -> impl PinInit<Self, i32> {
        // SAFETY:
        // - when the closure returns `Ok(())`, then it has successfully initialized and
        //   enabled `foo`,
        // - when it returns `Err(e)`, then it has cleaned up before
        unsafe {
            pin_init_from_closure(move |slot: *mut Self| {
                // `slot` contains uninit memory, avoid creating a reference.
                let foo = addr_of_mut!((*slot).foo);
                // Initialize the `foo`
                bindings::init_foo(UnsafeCell::raw_get(foo));
                // Try to enable it.
                let err = bindings::enable_foo(UnsafeCell::raw_get(foo), flags);
                if err != 0 {
                    // Enabling has failed, first clean up the foo and then return the error.
                    bindings::destroy_foo(UnsafeCell::raw_get(foo));
                    Err(err)
                } else {
                    // All fields of `RawFoo` have been initialized, since `_p` is a ZST.
                    Ok(())
                }
            })
        }
    }
}

impl Drop for RawFoo {
    fn drop(&mut self) {
        // SAFETY: since foo has been initialized, destroying is safe
        unsafe { bindings::destroy_foo(self.foo.get()) };
    }
}
```

For more information on how to use [`pin_init_from_closure()`], you can take a look at the
uses inside the `kernel` crate from the [Rust-for-Linux] project. The `sync` module is a good
starting point.

[structurally pinned fields]:
    https://doc.rust-lang.org/std/pin/index.html#pinning-is-structural-for-field
[stack]: crate::stack_pin_init
[`Arc<T>`]: alloc::sync::Arc
[`Box<T>`]: alloc::boxed::Box
[`impl PinInit<Foo>`]: PinInit
[`impl PinInit<T, E>`]: PinInit
[`impl Init<T, E>`]: Init
[`pin_data`]: ::pinned_init_macro::pin_data
[Rust-for-Linux]: https://rust-for-linux.com/
