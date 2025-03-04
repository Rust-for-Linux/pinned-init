use pin_init::*;

#[pin_data]
struct Array<const N: usize = 0> {
    array: [u8; N],
}

#[test]
fn create_array() {
    stack_pin_init!(let array: Array<1024> = init!(Array { array <- zeroed() }));
    println!("{}", array.array.len());
}
