use pin_init::*;

struct Foo {}

fn main() {
    let _ = init!(Foo {});
}
