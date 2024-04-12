use pinned_init::*;

#[pin_data]
struct Foo {
    #[pin]
    bar: Bar,
}

struct Bar;

impl Bar {
    fn new() -> impl PinInit<Self> {
        Self
    }
}

fn main() {
    let _ = init!(Foo {
        bar <- Bar::new(),
    });
}
