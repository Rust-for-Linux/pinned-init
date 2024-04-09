use pinned_init::*;

#[pin_data]
struct Foo {
    a: usize,
}

impl Drop for Foo {
    fn drop(&mut self) {}
}

fn main() {}
