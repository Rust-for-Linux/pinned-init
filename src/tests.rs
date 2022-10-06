/// ```rust,should_panic
/// #![feature(never_type)]
/// use pinned_init::*;
///
///
/// pin_data! {pub struct Thing {
///     a: usize,
/// }}
/// stack_init!(let a = pin_init!(Thing {
///     a: {return Ok::<_, !>(todo!()); 0}
/// }));
/// let a = a as Result<_, !>;
/// ```
fn deny_return1() {}

/// ```rust,compile_fail
/// #![feature(never_type)]
/// use pinned_init::*;
///
///
/// pin_data! {pub struct Thing {
///     a: usize,
/// }}
/// stack_init!(let a = pin_init!(Thing {
///     a: {return Ok::<__InitOk, !>(__InitOk); 0}
/// }));
/// let a = a as Result<_, !>;
/// ```
fn deny_return2() {}

/// ```rust,compile_fail
/// #![feature(never_type)]
/// use pinned_init::*;
///
///
/// pin_data! {pub struct Thing {
///     a: usize,
///     b: usize,
/// }}
/// stack_init!(let a = pin_init!(Thing {
///     a: 0,
/// }));
/// let a = a as Result<_, !>;
/// ```
fn deny_missing_field() {}

/// ```rust,compile_fail
/// #![feature(never_type)]
/// use pinned_init::*;
///
///
/// pin_data! {pub struct Thing {
///     a: usize,
///     b: usize,
/// }}
/// stack_init!(let a = pin_init!(Thing {
///     a: 0,
///     a: 0,
///     b: 0,
/// }));
/// let a = a as Result<_, !>;
/// ```
fn deny_duplicate_field() {}

/// ```rust
/// #![deny(warnings)]
/// #![feature(never_type)]
/// use pinned_init::*;
///
/// pin_data! {pub struct Thing {
///     a: usize,
///     b: usize,
/// }}
/// let a = pin_init!(Thing {
///     a: 0,
///     b: 0,
/// });
/// stack_init!(let b = a);
/// let b = b as Result<_, !>;
/// ```
fn demo() {}
