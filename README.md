# rs-auto-alloc

The global memory allocator for a Rust program, chosen per target **at
compile time**. You add one line to your binary:

```rust
rs_alloc::install!();
```

| Target and hardware | Default | Explicit candidates |
|---|---|---|
| Linux x86_64 (AMD/Intel) and AArch64, glibc | jemalloc | system, `linux-mimalloc`, `tcmalloc` |
| Linux x86_64 and AArch64, musl | jemalloc | system, `linux-mimalloc` |
| Linux ARM32 | mimalloc | system, `linux-mimalloc`, `jemalloc` |
| Linux RISC-V and other hosted architectures | system | system |
| Windows x86_64 / ARM64 | system | `mimalloc` |
| macOS Intel / Apple Silicon | system | `mimalloc`, `macos-jemalloc`, `tcmalloc` |
| Android ARM32 / ARM64 / x86_64 and iOS ARM64 (including the simulator) | system | `mobile-mimalloc` |
| Bare metal (`target_os = "none"`), including AVR, MSP430, RISC-V 32/64, Xtensa and ARM | embedded linked-list first-fit heap | board chooses heap size |
| WebAssembly with Rust's hosted standard library and other hosted targets | system | system |

The OS determines whether a system allocator exists. For example, an ARM Linux
process and ARM firmware require different backends. AMD and Intel share Rust's
`x86_64` target. CPU vendor alone gives no workload-specific winner.

## Using it

```toml
[dependencies]
rs-auto-alloc = "0.1.0"
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

## Measured automatic profile

For an unlocked application, enable `auto` and call the same `rs_alloc::install!()`.
This selects gperftools TCMalloc on GNU/Linux x86_64 and AArch64. Install
the gperftools development package on the build host and the shared library
on the deployment host. TCMalloc requires 64-bit pointers for this policy.
Linux ARM32 selects mimalloc both by default and with `--no-default-features --features auto`.
Version 0.1.0 used jemalloc on ARM32. Its small-batch benchmark took 42.85 µs versus
21.46 µs with upstream mimalloc on the same ARMv7 runner. [ARM32 measurements](https://github.com/codixor/rs-alloc/actions/runs/44899234883)
Windows and macOS (x86_64/AArch64), x86_64 Android
and the ARM64 iOS simulator use mimalloc. Physical ARM Android and iOS keep
the system allocator. Windows measurements used MSVC; the same backend choice
extends to Windows GNU on those architectures. Firmware still uses the board heap.

```toml
rs-auto-alloc = { version = "0.1.0", default-features = false, features = ["auto"] }
```

The desktop follow-up measured `auto` faster than upstream on Apple Silicon,
Intel macOS, Windows ARM64 and Windows x64. Windows x64 measured 1.036× upstream in the
four-target confirmation and 1.051× on a second runner. The original survey also found
gains on both Linux runners and both mobile simulation targets.
[Desktop confirmation](bench/DESKTOP-RESULTS.md) · [Original eight environments](bench/RESULTS.md)

## Selecting a backend

Disable the defaults for a hosted system build or for an isolated comparison.
For example, `default-features = false, features = ["tcmalloc"]` selects
gperftools TCMalloc on supported targets. Install the native development
library (`libgoogle-perftools-dev` on Debian/Ubuntu or `gperftools` on macOS).
The binding links `libtcmalloc`; set a native library search path for a
nonstandard prefix. This is the gperftools implementation.

Cargo features unify across the dependency graph. On a supported target, the
first applicable policy wins in this order: explicit TCMalloc, Linux mimalloc,
macOS jemalloc, desktop or mobile mimalloc, measured `auto`, then Linux jemalloc. The fallback
is the system.
The existing `mimalloc` flag applies only to Windows and macOS.
Use `mobile-mimalloc` to replace an Android or iOS system allocator explicitly. Use
`linux-mimalloc` only for processes that can tolerate its virtual memory
reservation. For Linux ARM32 services that need the previous jemalloc policy,
including those that use `mlockall`, set `default-features = false, features = ["jemalloc"]`.

The mobile and desktop backends ship in the same `rs-auto-alloc` crate. Features
that share a native dependency can compile it even when another allocator is
selected. For example, an ARM32 Linux default build also compiles jemalloc,
and a Windows or Android default build compiles mimalloc while selecting `System`.
For a hosted system-only build without native allocator dependencies, set
`default-features = false` and enable no allocator features.

When building mimalloc for iOS, set `IPHONEOS_DEPLOYMENT_TARGET` to your app’s
minimum iOS version so the Rust linker and native C build target the same OS.

## Firmware

The firmware entry point installs a named heap and initializes it once before
the first allocation. Supply the board's `critical-section` implementation
(e.g. through its HAL) and size the heap for the board's actual RAM budget.

```rust,ignore
rs_alloc::install!(HEAP);

// At early board startup, before interrupts or tasks can allocate:
// SAFETY: this is the only initialization and the macro owns the static storage.
unsafe { rs_alloc::init_heap!(HEAP, 1024); }
```

## Choosing for a workload

The upstream selector uses an OS split for mobile and bare metal and checks
core count for eligible desktops. Here the target and feature resolve the
allocator at compile time. No allocator hook probes the hardware. [Upstream selection logic](https://github.com/YeautyYE/auto-allocator/blob/28aea006da844756a19c91746dd994d7d765a3de/src/lib.rs)

* For a memory-locked Linux service, prioritize bounded locked memory and the
  existing regression test. For an unlocked concurrent server, compare jemalloc,
  mimalloc and TCMalloc against the system under the application's allocation mix.
  jemalloc offers arena and retention controls, mimalloc shards free lists by
  page, and gperftools TCMalloc caches small objects per thread.
  [jemalloc](https://jemalloc.net/jemalloc.3.html) · [mimalloc](https://github.com/microsoft/mimalloc) · [gperftools](https://gperftools.github.io/gperftools/tcmalloc.html)
* On Windows and macOS, compare the platform baseline under both single-thread
  churn and cross-thread frees. On mobile, check memory pressure and latency on
  a physical device before changing a shipping default.
* On microcontrollers, budget static RAM, interrupt latency and fragmentation.
  The first-fit heap uses the board's critical section. Firmware checks can
  establish build coverage; timings require a board and clock source.
  [embedded-alloc](https://github.com/rust-embedded/embedded-alloc)

Run the same comparison locally with Python 3, Cargo and the target toolchain:

```sh
python bench/run.py --target x86_64-unknown-linux-gnu --features system jemalloc linux-mimalloc tcmalloc --output target/bench/linux.json
```

The harness rotates executable order over five rounds on the same runner.
It measures a single producer, up to four concurrent producers and a bounded
cross-thread handoff. Each operation creates, writes and eventually frees a
32 B–32 KiB block. Reports contain throughput, p99 latency of a 128-operation
batch (producer-side for handoff), and process high-water RSS on Linux/Android.
Use the batch latency for comparisons within this benchmark. Application
request p99 requires an application-level test.

## Existing service measurements

- **Use it** in long-running services with many concurrent requests. Measured
  on a tokio secrets server, it served 35 to 50 % more requests per second
  than glibc when CPU-bound, with a quarter to a third lower p99.
- **Skip it** for command-line tools and short jobs: there is nothing to
  gain, and it adds a C build.
- **Never install it in a library.** Depending on `rs-auto-alloc` changes nothing;
  only a call to `install!()` does, and that belongs in a binary.

## Rules it keeps

1. **Compile time, never run time.** One binary runs one allocator on every
   host, in debug and release, so tests run what ships, and no block is ever
   freed by a different allocator than the one that made it.
2. **Only a binary installs it**, through `install!()`.
3. The desktop `auto` adapter uses mimalloc's ordinary, small, and aligned
   entry points according to Rust's requested layout. On macOS and Windows
   ARM64 it clears the requested extent for ordinary allocations up to
   64 KiB. Windows x64 uses mimalloc's native zeroing. Explicit `mimalloc`
   uses the stock Rust adapter. The gperftools adapter inherits the Rust
   trait defaults for resizing and zeroed allocations.
4. **Silent**: it prints nothing; log `rs_alloc::NAME` if you want.
5. **Memory-locked Linux services use jemalloc or the system allocator.** With overcommit, mimalloc commits a 1 GiB
   arena at the first allocation, and a process that calls `mlockall` locks
   all of it (measured: 1,054,596 KiB locked at start).
6. **Memory-locked processes get their memory back.** The kernel refuses
   `madvise` on locked pages, so jemalloc's default keeps every page it ever
   used; `retain:false` makes it unmap them instead (measured: a freed
   200 MiB block stayed locked without it, and was released with it).
7. Unsafe Rust is confined to the desktop adapter and its boundary tests,
   which cover zeroing, alignment, growth and shrinking. [Allocator source](src/lib.rs)

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
