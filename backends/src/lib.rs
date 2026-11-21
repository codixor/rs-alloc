#![no_std]
#![forbid(unsafe_code)]

#[cfg(all(feature = "macos-jemalloc", target_os = "macos"))]
pub use tikv_jemallocator::Jemalloc;

#[cfg(all(
    feature = "linux-mimalloc",
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub use mimalloc::MiMalloc;
