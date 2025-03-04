use pin_init::*;
struct Foo {}
fn main() {
    let _ = {
        struct __InitOk;
        let data = unsafe {
            use ::pin_init::__internal::HasInitData;
            Foo::__init_data()
        };
        let init = ::pin_init::__internal::InitData::make_closure::<
            _,
            __InitOk,
            ::core::convert::Infallible,
        >(
            data,
            move |slot| {
                {
                    struct __InitOk;
                    #[allow(unreachable_code, clippy::diverging_sub_expression)]
                    let _ = || unsafe { ::core::ptr::write(slot, Foo {}) };
                }
                Ok(__InitOk)
            },
        );
        let init = move |
            slot,
        | -> ::core::result::Result<(), ::core::convert::Infallible> {
            init(slot).map(|__InitOk| ())
        };
        let init = unsafe {
            ::pin_init::init_from_closure::<_, ::core::convert::Infallible>(init)
        };
        init
    };
}
