# rs-alloc

The global memory allocator for a Rust program, chosen per target **at
compile time**. You add one line to your binary:

```rust
rs_alloc::install!();
```

| Target | Allocator |
|---|---|
| Linux on x86_64 or aarch64 (glibc or musl), feature `jemalloc` (on by default) | jemalloc ([`tikv-jemallocator`](https://crates.io/crates/tikv-jemallocator)) |
| macOS or Windows, feature `mimalloc` (off by default) | mimalloc |
| every other target, or the features off | the system allocator |

## Using it

```toml
[dependencies]
rs-alloc = { git = "https://github.com/codixor/rs-alloc", rev = "<commit>" }
```

```rust
// src/main.rs of a long-running, concurrent service.
rs_alloc::install!();

fn main() {
    tracing::info!("memory allocator: {}", rs_alloc::NAME);
    // ...
}
```

Set jemalloc's build-time options in the program's `.cargo/config.toml`, and
install `make` where it builds (jemalloc's C code is built with configure +
make):

```toml
[env]
JEMALLOC_SYS_WITH_MALLOC_CONF = "narenas:4,retain:false"
```

On aarch64, also set `JEMALLOC_SYS_WITH_LG_PAGE=16` when building: jemalloc
compiles the page size in, and a 4 KiB build crashes on a 16 or 64 KiB page
kernel. 64 KiB pages also run on 4 and 16 KiB kernels.

## When to use it

- **Use it** in long-running services with many concurrent requests. Measured
  on a tokio secrets server, it served 35 to 50 % more requests per second
  than glibc when CPU-bound, with a quarter to a third lower p99.
- **Skip it** for command-line tools and short jobs: there is nothing to
  gain, and it adds a C build.
- **Never install it in a library.** Depending on `rs-alloc` changes nothing;
  only a call to `install!()` does, and that belongs in a binary.

## Rules it keeps

1. **Compile time, never run time.** One binary runs one allocator on every
   host, in debug and release, so tests run what ships, and no block is ever
   freed by a different allocator than the one that made it.
2. **Only a binary installs it**, through `install!()`.
3. **The allocator's own `GlobalAlloc`, whole**: `realloc` grows in place,
   zeroed blocks stay `calloc`.
4. **Silent**: it prints nothing; log `rs_alloc::NAME` if you want.
5. **Never mimalloc on Linux.** With overcommit, mimalloc commits a 1 GiB
   arena at the first allocation, and a process that calls `mlockall` locks
   all of it (measured: 1,054,596 KiB locked at start).
6. **Memory-locked processes get their memory back.** The kernel refuses
   `madvise` on locked pages, so jemalloc's default keeps every page it ever
   used; `retain:false` makes it unmap them instead (measured: a freed
   200 MiB block stayed locked without it, and was released with it).
7. **No `unsafe`** in this crate (`#![forbid(unsafe_code)]`).

## Tests

- `tests/selected.rs`: under the installed allocator, growth through
  `realloc`, shrinking, zeroed blocks (16 B to 8 MiB) and 4 KiB and 64 KiB
  alignment are correct. jemalloc really serves the program: an 8 MiB block
  starts on a cache line, where glibc puts it 16 bytes into a page
  (`tests/glibc_control.rs` is the control, and fails if jemalloc is
  preloaded).
- `tests/mlock.rs`: after `mlockall(MCL_CURRENT | MCL_FUTURE)`, locked memory
  is under 128 MiB at start and under 512 MiB after 8 threads churn 16 MiB
  each, and a freed 200 MiB block leaves the process. Where the memory-lock
  limit refuses `mlockall` it is skipped, unless `RS_ALLOC_REQUIRE_MLOCK` is
  set (CI raises the limit and sets it).

CI runs on Linux x86_64 and arm64 (built with 64 KiB pages), macOS and
Windows, plus the minimum Rust version (1.85), and runs `cargo-deny`.

## Licence

MIT OR Apache-2.0.
