use pinned_init::*;

struct Foo {}

fn main() {
    let _ = init!(Foo {});
}
