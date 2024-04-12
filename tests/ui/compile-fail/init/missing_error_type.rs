use pinned_init::*;

struct Foo {
    x: Box<usize>,
}

fn main() {
    let _ = try_init!(Foo { x: Box::new(0)? }?);
}
