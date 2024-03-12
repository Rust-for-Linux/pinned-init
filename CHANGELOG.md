# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- return type of `zeroed()` from `impl Init<T, E>` to `impl Init<T>` (also removing the generic
  parameter `E`)

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

[unreleased]: https://github.com/Rust-for-Linux/pinned-init/compare/v0.0.7...HEAD
[0.0.7]: https://github.com/Rust-for-Linux/pinned-init/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/Rust-for-Linux/pinned-init/releases/tag/v0.0.6
