use pinned_init::*;

#[pin_data]
struct Foo {
    a: Bar,
    b: Bar,
    c: Bar,
}

struct Bar;

fn main() {
    let _ = pin_init!(Foo {
        a: Bar,
        b: Bar
        c: Bar,
    });
}
