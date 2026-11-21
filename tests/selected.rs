//! The installed allocator serves the program's allocations, and its
//! `GlobalAlloc` behaves: growth, shrinking, zeroed and over-aligned blocks.

rs_alloc::install!();

#[test]
fn growth_shrink_zeroed_and_aligned_allocations_are_correct() {
    // Growth through realloc keeps the contents.
    let mut v: Vec<u64> = Vec::new();
    for i in 0..1_000_000u64 {
        v.push(i);
    }
    assert!(v.iter().enumerate().all(|(i, x)| *x == i as u64));
    v.truncate(10);
    v.shrink_to_fit();
    assert_eq!(v, (0..10).collect::<Vec<u64>>());

    // A zeroed allocation is zero, small and large.
    for size in [16usize, 4096, 8 << 20] {
        let z = vec![0u8; size];
        assert!(z.iter().all(|b| *b == 0));
    }

    // Over-aligned types are honoured. Space is asked for with
    // `with_capacity`, so no over-aligned value is ever built on the stack
    // (a 64 KiB-aligned stack frame is its own hazard on Windows).
    #[repr(align(4096))]
    struct Page(#[allow(dead_code)] [u8; 4096]);
    #[repr(align(65536))]
    struct Big(#[allow(dead_code)] [u8; 65536]);
    for n in [1usize, 3, 64] {
        let pages: Vec<Page> = Vec::with_capacity(n);
        assert_eq!(pages.as_ptr().addr() % 4096, 0);
        let big: Vec<Big> = Vec::with_capacity(n);
        assert_eq!(big.as_ptr().addr() % 65536, 0);
    }
}

/// Where a large block starts tells the allocators apart without reading
/// their internals: jemalloc starts large blocks on a cache line (at a
/// random cache line of the first page, its `cache_oblivious` option), while
/// glibc serves them with `mmap` behind a 16-byte header (the control is
/// `tests/glibc_control.rs`).
#[cfg(all(
    feature = "jemalloc",
    not(feature = "linux-mimalloc"),
    not(all(any(feature = "tcmalloc", feature = "auto"), target_env = "gnu")),
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn large_block_offset() -> usize {
    let v: Vec<u8> = Vec::with_capacity(8 << 20);
    v.as_ptr().addr() % 4096
}

#[cfg(all(
    feature = "jemalloc",
    not(feature = "linux-mimalloc"),
    not(all(any(feature = "tcmalloc", feature = "auto"), target_env = "gnu")),
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn jemalloc_serves_the_programs_allocations() {
    assert_eq!(rs_alloc::NAME, "jemalloc");
    for _ in 0..16 {
        let offset = large_block_offset();
        assert_eq!(
            offset % 64,
            0,
            "a large block at page offset {offset}, not on a cache line: not jemalloc"
        );
    }
}

/// Preserve the original feature contract for callers using the desktop defaults.
#[cfg(not(any(
    feature = "auto",
    feature = "mobile-mimalloc",
    feature = "tcmalloc",
    feature = "linux-mimalloc",
    feature = "macos-jemalloc"
)))]
#[test]
fn hosted_feature_selection() {
    let expected = if cfg!(all(
        feature = "jemalloc",
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        "jemalloc"
    } else if cfg!(all(
        feature = "mimalloc",
        any(target_os = "macos", target_os = "windows")
    )) {
        "mimalloc"
    } else {
        "system"
    };
    assert_eq!(rs_alloc::NAME, expected);
}

#[cfg(all(
    feature = "auto",
    not(any(
        feature = "tcmalloc",
        feature = "mobile-mimalloc",
        feature = "mimalloc",
        feature = "macos-jemalloc",
        feature = "linux-mimalloc"
    ))
))]
#[test]
fn auto_profile_selects_the_measured_backend() {
    let expected = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64" | "aarch64")
            if cfg!(all(target_env = "gnu", target_pointer_width = "64")) =>
        {
            "tcmalloc"
        }
        ("macos" | "windows", "x86_64" | "aarch64") | ("android", "x86_64") => "mimalloc",
        ("ios", "aarch64") if cfg!(target_abi = "sim") => "mimalloc",
        ("linux", "x86_64" | "aarch64") if cfg!(feature = "jemalloc") => "jemalloc",
        _ => "system",
    };
    assert_eq!(rs_alloc::NAME, expected);
}

#[cfg(all(
    feature = "mobile-mimalloc",
    any(target_os = "android", target_os = "ios")
))]
#[test]
fn mobile_replacement_is_explicit() {
    assert_eq!(rs_alloc::NAME, "mimalloc");
}
