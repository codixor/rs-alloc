//! The program's memory allocator, chosen per target at compile time.
//!
//! A binary installs it with one line at the top of its `main.rs`:
//!
//! ```ignore
//! rs_alloc::install!();
//! ```
//!
//! | Target | Allocator |
//! |---|---|
//! | Linux, x86_64 or aarch64 (glibc or musl), feature `jemalloc` (default) | jemalloc |
//! | macOS or Windows, feature `mimalloc` (off by default) | mimalloc |
//! | everything else, or the features off | the system allocator |
//!
//! Rules this crate keeps:
//!
//! - **Chosen when compiling, never at run time.** One binary always runs
//!   one allocator, on every host, in debug and release alike, so tests run
//!   what ships, and no allocation can be freed by a different allocator.
//! - **A library never installs it.** This crate only declares the choice;
//!   the `#[global_allocator]` static exists only where a binary calls
//!   [`install!`]. Depending on this crate changes nothing by itself.
//! - **The allocator's own `GlobalAlloc` is used whole**, so `realloc`
//!   grows in place and zeroed allocations stay `calloc`.
//! - **Silent.** Nothing is printed; the program logs [`NAME`] if it wants.
//! - **No mimalloc on Linux.** With overcommit it commits a 1 GiB arena at
//!   the first allocation, and a process that calls `mlockall` (a hardened
//!   server or agent) locks all of it.
//! - **No `unsafe`** here: the statics are safe code.
//!
//! Build settings that only the build can make, because jemalloc reads them
//! when its C code is compiled (tikv-jemalloc-sys build script):
//!
//! - `JEMALLOC_SYS_WITH_MALLOC_CONF=narenas:4,retain:false` (in the
//!   program's `.cargo/config.toml` `[env]`). `retain:false` is required in
//!   a process that calls `mlockall`: the kernel refuses `madvise` on locked
//!   pages, so jemalloc's default (`retain:true`, which returns memory only
//!   through `madvise`) keeps every page it ever used until the process
//!   exits; with `retain:false` it unmaps them. `narenas:4` keeps fewer
//!   arenas, so less memory stays locked. `tests/mlock.rs` fails without
//!   `retain:false`.
//! - `JEMALLOC_SYS_WITH_LG_PAGE=16` when building for aarch64: jemalloc
//!   compiles the page size in, and a 4 KiB build crashes on a 16 or 64 KiB
//!   page kernel; 64 KiB works on all three.

#![forbid(unsafe_code)]

/// Whether this build uses jemalloc.
macro_rules! jemalloc_target {
    ($($item:item)*) => {$(
        #[cfg(all(feature = "jemalloc", target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]
        $item
    )*};
}
/// Whether this build uses mimalloc.
macro_rules! mimalloc_target {
    ($($item:item)*) => {$(
        #[cfg(all(feature = "mimalloc", any(target_os = "macos", target_os = "windows")))]
        $item
    )*};
}
/// Whether this build keeps the system allocator.
macro_rules! system_target {
    ($($item:item)*) => {$(
        #[cfg(not(any(
            all(feature = "jemalloc", target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")),
            all(feature = "mimalloc", any(target_os = "macos", target_os = "windows")),
        )))]
        $item
    )*};
}

jemalloc_target! {
    /// The allocator type this build installs.
    pub type Allocator = tikv_jemallocator::Jemalloc;
    /// The allocator this build installs.
    pub const ALLOCATOR: Allocator = tikv_jemallocator::Jemalloc;
    /// Its name, for a start-up log line.
    pub const NAME: &str = "jemalloc";
}
mimalloc_target! {
    /// The allocator type this build installs.
    pub type Allocator = mimalloc::MiMalloc;
    /// The allocator this build installs.
    pub const ALLOCATOR: Allocator = mimalloc::MiMalloc;
    /// Its name, for a start-up log line.
    pub const NAME: &str = "mimalloc";
}
system_target! {
    /// The allocator type this build installs.
    pub type Allocator = std::alloc::System;
    /// The allocator this build installs.
    pub const ALLOCATOR: Allocator = std::alloc::System;
    /// Its name, for a start-up log line.
    pub const NAME: &str = "system";
}

/// Installs [`ALLOCATOR`] as the program's global allocator. Call it once,
/// at the top level of a binary's `main.rs`; never in a library.
#[macro_export]
macro_rules! install {
    () => {
        #[global_allocator]
        static RS_ALLOC_GLOBAL_ALLOCATOR: $crate::Allocator = $crate::ALLOCATOR;
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn name_matches_the_target() {
        let jemalloc = cfg!(all(
            feature = "jemalloc",
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ));
        let mimalloc = cfg!(all(
            feature = "mimalloc",
            any(target_os = "macos", target_os = "windows")
        ));
        let expected = if jemalloc {
            "jemalloc"
        } else if mimalloc {
            "mimalloc"
        } else {
            "system"
        };
        assert_eq!(super::NAME, expected);
    }
}
