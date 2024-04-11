use pinned_init::*;

struct Foo<T> {
    value: T,
}
fn main() {
    let _ = init!(Foo<()> {
        value <- (),
    });
}
