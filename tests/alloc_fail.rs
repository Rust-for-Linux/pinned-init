#![feature(allocator_api)]

use core::alloc::AllocError;
use pinned_init::*;
use std::sync::Arc;

#[path = "./ring_buf.rs"]
mod ring_buf;
use ring_buf::*;

#[cfg(all(not(miri), not(NO_ALLOC_FAIL_TESTS), not(target_os = "macos")))]
#[test]
fn too_big_pinned() {
    // should be too big with current hardware.
    assert!(matches!(
        Box::pin_init(RingBuffer::<u8, { 1024 * 1024 * 1024 * 1024 }>::new()),
        Err(AllocError)
    ));
    // should be too big with current hardware.
    assert!(matches!(
        Arc::pin_init(RingBuffer::<u8, { 1024 * 1024 * 1024 * 1024 }>::new()),
        Err(AllocError)
    ));
}

#[cfg(all(not(miri), not(NO_ALLOC_FAIL_TESTS), not(target_os = "macos")))]
#[test]
fn too_big_in_place() {
    // should be too big with current hardware.
    assert!(matches!(
        Box::init(zeroed::<[u8; 1024 * 1024 * 1024 * 1024]>()),
        Err(AllocError)
    ));
    // should be too big with current hardware.
    assert!(matches!(
        Arc::init(zeroed::<[u8; 1024 * 1024 * 1024 * 1024]>()),
        Err(AllocError)
    ));
}
