use pin_init::*;

#[pin_data]
struct Foo {
    a: Box<Foo>
    b: Box<Foo>
}
