//! The control for `tests/selected.rs`: with the features off, the same
//! check sees glibc, so that test can tell the two apart. Its own file, so
//! its own process: once glibc has freed an `mmap`ed block it raises its
//! `mmap` threshold, and a later large block comes from the heap instead.

#![cfg(all(
    not(any(
        feature = "auto",
        feature = "jemalloc",
        feature = "linux-mimalloc",
        feature = "tcmalloc"
    )),
    target_os = "linux",
    target_env = "gnu",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

rs_alloc::install!();

/// Where a large block starts tells the allocators apart without reading
/// their internals: jemalloc starts large blocks on a cache line (at a
/// random cache line of the first page, its `cache_oblivious` option), while
/// glibc serves them with `mmap` behind a 16-byte header.
fn large_block_offset() -> usize {
    let v: Vec<u8> = Vec::with_capacity(8 << 20);
    v.as_ptr().addr() % 4096
}

#[test]
fn glibc_serves_the_programs_allocations_without_the_feature() {
    assert_eq!(rs_alloc::NAME, "system");
    assert_eq!(
        large_block_offset(),
        16,
        "a large block without glibc's 16-byte mmap header"
    );
}
