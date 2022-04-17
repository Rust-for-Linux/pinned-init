
#[pinned_init]
#[repr(transparent)]
pub struct LinkedList<T> {
    #[init]
    inner: Inner<T>,
}

#[manual_init]
#[repr(C)]
struct Inner<T> {
    #[init]
    prev: StaticUninit<*const LinkedList<T>>,
    #[init]
    next: StaticUninit<*const LinkedList<T>>,
    phantom: PhantomData<fn(T) -> T>,
}

impl<T> PinnedInit for Inner<T>
