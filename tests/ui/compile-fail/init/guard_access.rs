use pinned_init::*;

struct Foo {
    x: usize,
    y: usize,
}

fn main() {
    let _ = init!(Foo {
        x: 0,
        y: {
            let _ = __x_guard;
            0
        },
    });
}
