use pin_init::*;

#[derive(Zeroable)]
struct Foo {
    a: usize,
    b: usize,
}

fn main() {
    let _ = init!(Foo {
        a: 0,
        ..Zeroable::zeroed(),
    });
}
