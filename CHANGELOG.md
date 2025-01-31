# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `InPlaceInit` now only exists when the `alloc` or `std` features are enabled

## [0.0.9] - 2024-12-02

### Added

- `InPlaceWrite` trait to re-initialize already existing allocations,
- `assert_pinned!` macro to check if a field is marked with `#[pin]`,
- compatibility with stable Rust, thanks a lot to @bonzini! #24 and #23:
  - the `alloc` feature enables support for `allocator_api` and reflects the old behavior, if it is
    disabled, then infallible allocations are assumed (just like the standard library does).

### Fixed

- guard hygiene wrt constants in `[try_][pin_]init!`

## [0.0.8] - 2024-07-07

### Changed

- return type of `zeroed()` from `impl Init<T, E>` to `impl Init<T>` (also removing the generic
  parameter `E`)
- removed the default error of `try_[pin_]init!`, now you always have to specify an error using
  `? Error` at the end
- put `InPlaceInit` behind the `alloc` feature flag, this allows stable usage of the `#![no_std]`
  part of the crate

## [0.0.7] - 2024-04-09

### Added

- `Zeroable` derive macro
- `..Zeroable::zeroed()` tail expression support in `[try_][pin_]init!` macros: allowed to omit
  fields, omitted fields are initialized with `0`
- `[pin_]chain` functions to modify a value after an initializer has run
- `[pin_]init_array_from_fn` to create `impl [Pin]Init<[T; N], E>` from a generator closure
  `fn(usize) -> impl [Pin]Init<T, E>`
- `impl Zeroable for UnsafeCell`

### Changed

- `PinInit` is now a supertrait of `Init` (before there was a blanket impl) 

### Removed

- coverage workflow and usage of `#[feature(no_coverage)]`
- `impl Zeroable for Infallible` (see [Security](#security))

### Fixed

- `Self` in generic bounds on structs with `#[pin_data]`
- const generic default parameter values can now be used on structs with `#[pin_data]`

### Security

- `impl Zeroable for Infallible` (#13) it was possible to trigger UB by creating a value of type
  `Box<Infallible>` via `Box::init(zeroed())`

## [0.0.6] - 2023-04-08

[unreleased]: https://github.com/Rust-for-Linux/pinned-init/compare/v0.0.9...HEAD
[0.0.9]: https://github.com/Rust-for-Linux/pinned-init/compare/v0.0.8...v0.0.9
[0.0.8]: https://github.com/Rust-for-Linux/pinned-init/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/Rust-for-Linux/pinned-init/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/Rust-for-Linux/pinned-init/releases/tag/v0.0.6
