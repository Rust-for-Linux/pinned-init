use pinned_init::*;

struct Foo {
    a: usize,
}

fn main() {
    let _ = init!(Foo { a: () });
}
