//! Compile-time global allocator selection. A binary opts in with [`install!`].
//! Allocation calls use the selected backend implementation of `GlobalAlloc`. Debug and
//! release builds use the same policy. See the README for targets and features.
//!
//! Linux ARM32 defaults to mimalloc. Linux AArch64 and x86_64 default to jemalloc.
//! On ARM32, select jemalloc with `default-features = false, features = ["jemalloc"]`.
//! For memory-locked services, build jemalloc with `JEMALLOC_SYS_WITH_MALLOC_CONF=narenas:4,retain:false`.
//! Use `JEMALLOC_SYS_WITH_LG_PAGE=16` for portable AArch64 jemalloc binaries.
//! The opt-in `auto` profile follows the target policy in the README.
//! Other hosted targets default to the system allocator. Firmware supplies
//! its own heap storage and critical-section implementation.

#![cfg_attr(target_os = "none", no_std)]

#[cfg(not(target_os = "none"))]
macro_rules! hosted_allocator {
    ($backend:path, $name:literal) => {
        /// Allocator type installed by this build.
        pub use $backend as Allocator;
        /// Value installed by [`install!`].
        pub const ALLOCATOR: Allocator = Allocator;
        /// Selected backend, suitable for startup logs.
        pub const NAME: &str = $name;
    };
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "none")] {
        /// Board heap. Initialize the installed instance once during startup.
        pub use embedded_alloc::LlffHeap as Allocator;
        /// Initialize the named heap from a board-sized static buffer.
        pub use embedded_alloc::init as init_heap;
        /// Selected firmware allocator.
        pub const NAME: &str = "embedded-llff";
    } else if #[cfg(all(
        feature = "tcmalloc",
        any(target_arch = "x86_64", target_arch = "aarch64"),
        any(all(target_os = "linux", target_env = "gnu"), target_os = "macos")
    ))] {
        hosted_allocator!(tcmalloc::TCMalloc, "tcmalloc");
    } else if #[cfg(all(
        target_os = "linux",
        any(
            all(target_arch = "arm", any(feature = "default", feature = "auto", feature = "linux-mimalloc")),
            all(feature = "linux-mimalloc", any(target_arch = "x86_64", target_arch = "aarch64"))
        )
    ))] {
        hosted_allocator!(mimalloc::MiMalloc, "mimalloc");
    } else if #[cfg(all(feature = "macos-jemalloc", target_os = "macos"))] {
        hosted_allocator!(tikv_jemallocator::Jemalloc, "jemalloc");
    } else if #[cfg(all(
        feature = "mimalloc",
        any(target_os = "macos", target_os = "windows")
    ))] {
        hosted_allocator!(mimalloc::MiMalloc, "mimalloc");
    } else if #[cfg(any(
        all(feature = "mobile-mimalloc", any(target_os = "ios", target_os = "android")),
        all(feature = "auto", target_os = "android", target_arch = "x86_64"),
        all(feature = "auto", target_os = "ios", target_arch = "aarch64", target_abi = "sim")
    ))] {
        hosted_allocator!(mimalloc::MiMalloc, "mimalloc");
    } else if #[cfg(all(
        feature = "auto", target_os = "linux", target_env = "gnu", target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))] {
        hosted_allocator!(tcmalloc::TCMalloc, "tcmalloc");
    } else if #[cfg(all(
        feature = "auto", any(target_os = "macos", target_os = "windows"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))] {
        #[allow(unsafe_code)]
        mod desktop {
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
                    // On modest, naturally aligned requests LLVM can clear precisely the
                    // Rust layout. Keep mimalloc's page-aware zeroing for larger blocks.
                    const CLEAR_REQUESTED_BYTES: bool = cfg!(any(
                        target_os = "macos",
                        all(target_os = "windows", target_arch = "aarch64")
                    ));
                    if CLEAR_REQUESTED_BYTES
                        && ordinary(layout.size(), layout.align())
                        && layout.size() <= 64 * 1024
                    {
                        // SAFETY: alloc returns null or a live, writable block of layout.size() bytes.
                        let block = unsafe { self.alloc(layout) };
                        if !block.is_null() {
                            unsafe { block.write_bytes(0, layout.size()) };
                        }
                        return block;
                    }
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

            #[cfg(test)]
            mod tests {
                use super::*;
                use core::{ptr, slice};

                #[test]
                fn zeroing_and_resize_preserve_layout_at_native_boundaries() {
                    let limit = mi::MI_SMALL_SIZE_MAX;
                    for align in [1, 2, 8, 16, 64, 4096, 65536] {
                        for size in [
                            1,
                            7,
                            8,
                            17,
                            limit - 1,
                            limit,
                            limit + 1,
                            8193,
                            65535,
                            65536,
                            65537,
                        ] {
                            let layout = Layout::from_size_align(size, align).unwrap();
                            let grown_size = size + limit;
                            let grown_layout = Layout::from_size_align(grown_size, align).unwrap();
                            // SAFETY: Layouts are nonempty and valid. We only access initialized
                            // bytes inside live allocations. Each successful resize transfers
                            // ownership and we free the final pointer with its current layout.
                            unsafe {
                                let dirty = DesktopMiMalloc.alloc(layout);
                                assert!(!dirty.is_null());
                                assert_eq!(dirty.addr() % align, 0);
                                ptr::write_bytes(dirty, 0xff, size);
                                DesktopMiMalloc.dealloc(dirty, layout);

                                let block = DesktopMiMalloc.alloc_zeroed(layout);
                                assert!(!block.is_null());
                                assert_eq!(block.addr() % align, 0);
                                assert!(slice::from_raw_parts(block, size).iter().all(|&b| b == 0));
                                ptr::write_bytes(block, 0xa5, size);

                                let grown = DesktopMiMalloc.realloc(block, layout, grown_size);
                                assert!(!grown.is_null());
                                assert_eq!(grown.addr() % align, 0);
                                assert!(
                                    slice::from_raw_parts(grown, size)
                                        .iter()
                                        .all(|&b| b == 0xa5)
                                );

                                let small = DesktopMiMalloc.realloc(grown, grown_layout, 1);
                                assert!(!small.is_null());
                                assert_eq!(small.addr() % align, 0);
                                assert_eq!(small.read(), 0xa5);
                                DesktopMiMalloc.dealloc(small, Layout::from_size_align(1, align).unwrap());
                            }
                        }
                    }
                }
            }
        }
        hosted_allocator!(desktop::DesktopMiMalloc, "mimalloc");
    } else if #[cfg(all(
        target_os = "linux", feature = "jemalloc",
        any(target_arch = "arm", target_arch = "x86_64", target_arch = "aarch64")
    ))] {
        hosted_allocator!(tikv_jemallocator::Jemalloc, "jemalloc");
    } else {
        hosted_allocator!(std::alloc::System, "system");
    }
}

/// Install the selected allocator once in the application binary.
#[cfg(not(target_os = "none"))]
#[macro_export]
macro_rules! install {
    () => {
        #[global_allocator]
        static RS_ALLOC_GLOBAL_ALLOCATOR: $crate::Allocator = $crate::ALLOCATOR;
    };
}

/// Declare a named global heap in firmware. Initialize that same static with
/// `init_heap!(HEAP, BYTES)` before allocating. The board supplies the
/// critical-section implementation and chooses a size that fits its RAM.
#[cfg(target_os = "none")]
#[macro_export]
macro_rules! install {
    ($heap:ident) => {
        #[global_allocator]
        static $heap: $crate::Allocator = $crate::Allocator::empty();
    };
}
