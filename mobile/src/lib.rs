#![no_std]
#![forbid(unsafe_code)]

#[cfg(all(
    any(target_os = "android", target_os = "ios"),
    any(
        feature = "explicit",
        all(target_os = "android", target_arch = "x86_64"),
        all(target_os = "ios", target_arch = "aarch64", target_abi = "sim")
    )
))]
pub use mimalloc::MiMalloc;
