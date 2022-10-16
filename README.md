Library to safely and fallibly initialize pinned structs using in-place constructors.

It also allows in-place initialization of big structs that would otherwise produce a stack overflow.

[Pinning][pinning] is Rust's way of ensuring data does not move.

# Overview

To initialize a struct with an in-place constructor you will need two things:
- an in-place constructor,
- a memory location that can hold your struct (this can be the stack, an `Arc<T>`,
  `Box<T>` or any other smart pointer [^1]).

To get an in-place constructor there are generally two options:
- directly creating an in-place constructor,
- a function/macro returning an in-place constructor.

# Examples

## Directly creating an in-place constructor

If you want to use `PinInit`, then you will have to annotate your struct with `#[pin_project]`.
It is a macro that uses `#[pin]` as a marker for [structurally pinned fields].

```rust
#[pin_project]
struct Foo {
    #[pin]
    a: Mutex<usize>,
    b: u32,
}

let foo = pin_init!(Foo {
    a: Mutex::new(42),
    b: 24,
});
```

`foo` now is of the type `impl PinInit<Foo>`. We can now use any smart pointer that we like
(or just the stack) to actually initialize a `Foo`:

```rust
let foo: Result<Pin<Box<Foo>>, _> = Box::pin_init::<core::convert::Infallible>(foo);
```

## Using a function/macro that returns an initializer

Many types using this library supply a function/macro that returns an initializer, because the
above method only works for types where you can access the fields.

```rust
let mtx: Result<Pin<Arc<Mutex<usize>>>, _> = Arc::pin_init(Mutex::new(42));
```

To declare an init macro/function you just return an `impl PinInit<T, E>`:
```rust
#[pin_project]
struct DriverData {
    #[pin]
    status: Mutex<i32>,
    buffer: Box<[u8; 1_000_000]>,
}

impl DriverData {
    fn new() -> impl PinInit<Self, AllocOrInitError<Infallible>> {
        pin_init!(Self {
            status: Mutex::new(0),
            buffer: Box::init(pinned_init::zeroed())?,
        })
    }
}
```


[^1]: That is not entirely true, only smart pointers that implement `InPlaceInit`.

[pinning]: https://doc.rust-lang.org/std/pin/index.html
[structurally pinned fields]: https://doc.rust-lang.org/std/pin/index.html#pinning-is-structural-for-field
