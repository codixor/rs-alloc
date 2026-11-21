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

#[cfg(all(
    feature = "auto",
    any(
        all(
            any(target_os = "macos", target_os = "windows"),
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "android", target_arch = "x86_64"),
        all(target_os = "ios", target_arch = "aarch64", target_abi = "sim")
    )
))]
pub use mimalloc::MiMalloc;
