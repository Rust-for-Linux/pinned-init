use pinned_init::*;

#[pin_data]
struct Foo {
    bar: Bar,
}

#[pin_data]
struct Bar {
    a: usize,
}

impl Bar {
    fn new() -> impl PinInit<Self> {
        pin_init!(Self { a: 42 })
    }
}

impl Foo {
    fn new() -> impl PinInit<Self> {
        pin_init!(Self { bar: Bar::new() })
    }
}

fn main() {}
