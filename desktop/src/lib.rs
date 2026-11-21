//! Rust layout dispatch for the native mimalloc allocator.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

use core::alloc::{GlobalAlloc, Layout};
use core::mem::align_of;
use libmimalloc_sys as mi;

/// Stateless mimalloc adapter for 64-bit desktop applications.
pub struct DesktopMiMalloc;

// A pointer-sized word is the minimum alignment of mimalloc's ordinary
// small blocks on our desktop targets. Use the aligned API for stricter
// layouts, including when the new size is smaller than the alignment.
#[inline]
fn ordinary(size: usize, align: usize) -> bool {
    align <= align_of::<usize>() && align <= size
}

// SAFETY: Every method uses the same mimalloc instance. For ordinary layouts
// its native blocks meet the requested alignment. Other layouts retain their
// explicit alignment, including across realloc. Native failures return null.
unsafe impl GlobalAlloc for DesktopMiMalloc {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc supplies a valid nonempty layout. The small API
        // requires its size bound to be checked before entering C.
        unsafe {
            if !ordinary(layout.size(), layout.align()) {
                mi::mi_malloc_aligned(layout.size(), layout.align())
            } else if layout.size() <= mi::MI_SMALL_SIZE_MAX {
                mi::mi_malloc_small(layout.size())
            } else {
                mi::mi_malloc(layout.size())
            }
        }
        .cast()
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: As in alloc, each branch respects the alignment and the
        // small API's size bound. Each native entry point zeroes the block.
        unsafe {
            if !ordinary(layout.size(), layout.align()) {
                mi::mi_zalloc_aligned(layout.size(), layout.align())
            } else if layout.size() <= mi::MI_SMALL_SIZE_MAX {
                mi::mi_zalloc_small(layout.size())
            } else {
                mi::mi_zalloc(layout.size())
            }
        }
        .cast()
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: The caller owns the live mimalloc block. Both APIs preserve
        // the common prefix and leave the original allocation live on failure.
        unsafe {
            if ordinary(new_size, layout.align()) {
                mi::mi_realloc(ptr.cast(), new_size)
            } else {
                mi::mi_realloc_aligned(ptr.cast(), new_size, layout.align())
            }
        }
        .cast()
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // SAFETY: mi_free accepts every block returned by the APIs above,
        // including aligned blocks and blocks reallocated on another thread.
        unsafe { mi::mi_free(ptr.cast()) }
    }
}
