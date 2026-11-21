//! Compile-time global allocator selection. A binary opts in with [`install!`].
//! Allocation calls use the selected backend implementation of `GlobalAlloc`. Debug and
//! release builds use the same policy. See the README for targets and features.
//!
//! Linux x86_64 and AArch64 default to jemalloc. Memory-locked services should
//! build it with `JEMALLOC_SYS_WITH_MALLOC_CONF=narenas:4,retain:false`.
//! Use `JEMALLOC_SYS_WITH_LG_PAGE=16` for portable AArch64 jemalloc binaries.
//! The opt-in `auto` profile follows the measured target policy in the README.
//! Other hosted targets default to the system allocator. Firmware supplies
//! its own heap storage and critical-section implementation.

#![cfg_attr(target_os = "none", no_std)]
#![forbid(unsafe_code)]

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
        feature = "linux-mimalloc", target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))] {
        hosted_allocator!(alternative_backends::MiMalloc, "mimalloc");
    } else if #[cfg(all(feature = "macos-jemalloc", target_os = "macos"))] {
        hosted_allocator!(alternative_backends::Jemalloc, "jemalloc");
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
        hosted_allocator!(rs_alloc_mobile::MiMalloc, "mimalloc");
    } else if #[cfg(all(
        feature = "auto", target_os = "linux", target_env = "gnu", target_pointer_width = "64",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))] {
        hosted_allocator!(tcmalloc::TCMalloc, "tcmalloc");
    } else if #[cfg(all(
        feature = "auto", any(target_os = "macos", target_os = "windows"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))] {
        hosted_allocator!(rs_alloc_desktop::DesktopMiMalloc, "mimalloc");
    } else if #[cfg(all(
        feature = "jemalloc", target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
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
