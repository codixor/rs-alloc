//! Under a hardened server's `mlockall(MCL_CURRENT | MCL_FUTURE)`, the
//! installed allocator stays far inside a 1 GiB memory limit: at start, and after a concurrent burst of
//! allocations. mimalloc fails the first check (1 GiB locked at start).
//!
//! Locking needs a memory-lock limit the default one is too small for. When
//! `mlockall` is refused the test says so and passes, unless
//! `RS_ALLOC_REQUIRE_MLOCK` is set, as in CI, where it must run.

#![cfg(target_os = "linux")]

rs_alloc::install!();

use rustix::mm::{MlockAllFlags, mlockall};

fn status_kb(key: &str) -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap();
    let line = s.lines().find(|l| l.starts_with(key)).unwrap();
    line.split_whitespace().nth(1).unwrap().parse().unwrap()
}

#[test]
fn locked_memory_stays_bounded() {
    if let Err(e) = mlockall(MlockAllFlags::CURRENT | MlockAllFlags::FUTURE) {
        assert!(
            std::env::var_os("RS_ALLOC_REQUIRE_MLOCK").is_none(),
            "mlockall refused: {e}"
        );
        eprintln!("skipped: mlockall refused ({e}); raise the memlock limit to run this test");
        return;
    }
    // A first allocation, then the start-up footprint.
    let warm = vec![1u8; 4096];
    // VmLck is the size of the locked mappings, reserved address space
    // included; VmRSS is what is resident, and with MCL_FUTURE every page an
    // allocator makes usable is faulted in and locked, so it is the measure.
    assert!(status_kb("VmLck:") > 0, "nothing locked after mlockall");
    let start = status_kb("VmRSS:");
    // The test binary's own mappings are locked too. A 1 GiB arena fails
    // this by far.
    assert!(
        start < 128 * 1024,
        "{} locks {start} KiB at start",
        rs_alloc::NAME
    );
    drop(warm);

    // A burst: 8 threads, each keeping 256 live blocks of 64 B to 64 KiB and
    // replacing them 40 times over (about 16 MiB live per thread).
    let threads: Vec<_> = (0..8u32)
        .map(|t| {
            std::thread::spawn(move || {
                let mut live: Vec<Vec<u8>> = Vec::with_capacity(256);
                let mut x = 0x9E37_79B9u32.wrapping_mul(t + 1);
                for round in 0..40u32 {
                    live.clear();
                    for _ in 0..256 {
                        x ^= x << 13;
                        x ^= x >> 17;
                        x ^= x << 5;
                        let size = 64 + (x as usize % (64 * 1024));
                        live.push(vec![round as u8; size]);
                    }
                }
                live.iter().map(Vec::len).sum::<usize>()
            })
        })
        .collect();
    let live: usize = threads.into_iter().map(|h| h.join().unwrap()).sum();
    assert!(live > 0);
    let after = status_kb("VmRSS:");
    eprintln!(
        "{}: {start} KiB resident and locked at start, {after} KiB after the burst",
        rs_alloc::NAME
    );
    assert!(
        after < 512 * 1024,
        "{} locks {after} KiB after the burst",
        rs_alloc::NAME
    );

    // Memory freed must leave the process. Locked pages cannot be released
    // with madvise (EINVAL), so an allocator that only purges that way keeps
    // every page it ever held: jemalloc does, unless built with
    // `retain:false` (JEMALLOC_SYS_WITH_MALLOC_CONF), which unmaps instead.
    let held = vec![1u8; 200 << 20];
    let with_block = status_kb("VmRSS:");
    drop(held);
    // A few small allocations give jemalloc's decay a chance to run.
    for _ in 0..1000 {
        std::hint::black_box(vec![0u8; 4096]);
    }
    let freed = status_kb("VmRSS:");
    eprintln!(
        "{}: {with_block} KiB with a 200 MiB block, {freed} KiB after freeing it",
        rs_alloc::NAME
    );
    assert!(
        freed + 150 * 1024 < with_block,
        "{} kept a freed 200 MiB block locked ({with_block} -> {freed} KiB); for jemalloc, build with JEMALLOC_SYS_WITH_MALLOC_CONF=retain:false",
        rs_alloc::NAME
    );
}
