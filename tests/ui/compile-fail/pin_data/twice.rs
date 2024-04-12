use pinned_init::*;

#[pin_data]
#[pin_data]
struct Foo {
    #[pin]
    a: usize,
}

fn main() {}
