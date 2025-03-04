use core::{marker::PhantomPinned, pin::Pin};
use pin_init::*;

trait Bar<'a, const ID: usize = 0> {
    fn bar(&mut self);
}

#[pin_data(PinnedDrop)]
struct Foo<'a, 'b: 'a, T: Bar<'b> + ?Sized + 'a, const SIZE: usize = 0>
where
    T: Bar<'a, 1>,
{
    _array: [u8; 1024 * 1024],
    r: &'b mut [&'a mut T; SIZE],
    #[pin]
    _pin: PhantomPinned,
}

#[pinned_drop]
impl<'a, 'b: 'a, T: Bar<'b> + ?Sized + 'a, const SIZE: usize> PinnedDrop for Foo<'a, 'b, T, SIZE>
where
    T: Bar<'b, 1>,
{
    fn drop(self: Pin<&mut Self>) {
        // SAFETY: we do not move out of `self`
        let me = unsafe { Pin::get_unchecked_mut(self) };
        for t in &mut *me.r {
            Bar::<'a, 1>::bar(*t);
        }
    }
}

fn main() {}
