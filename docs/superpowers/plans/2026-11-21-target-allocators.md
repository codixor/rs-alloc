# Target allocator and upstream comparison implementation plan

**Goal** — Deliver a compile-time Rust allocator for the requested operating systems and firmware targets, measure it against the upstream suite and audit the result.

**Architecture** — The binary installs one concrete allocator. Hosted builds choose a backend with Cargo features. Firmware owns its heap size and critical section. The comparison compiles the pinned upstream workload into separate binaries and rotates their execution order on the same machine.

**Toolchain** — Rust 2024, MSRV 1.85, Cargo, Criterion 0.5.1, Python 3, GitHub-hosted runners and ECC for Codex.

**Scope** — The [upstream selector](https://github.com/YeautyYE/auto-allocator/blob/28aea006da844756a19c91746dd994d7d765a3de/src/lib.rs) supplies the OS policy reference. The [benchmark](https://github.com/YeautyYE/auto-allocator/blob/28aea006da844756a19c91746dd994d7d765a3de/benches/allocator_benchmark.rs) defines 21 timed cases. The user selected hosted runners and requested removal of the temporary CI after testing.

## Constraints and review focus

- Preserve the one-line binary install API, the silent hot path and existing memory-locked Linux defaults.
- Check aggregate feature resolution and ensure existing consumers do not acquire an unused native toolchain dependency.
- Compile a downstream firmware consumer that initializes the same named heap it installs.
- Require the intended allocator in measured executables and retain medians for all 21 cases including losses.
- Distinguish mobile simulation, cross-compilation and physical execution. Record the actual runner CPU.
- Verify that the final checkout and remote refs contain no temporary benchmark automation.

## 1. Target selection

- [x] Extend `Cargo.toml` and `src/lib.rs` for gperftools TCMalloc, explicit Linux mimalloc, macOS jemalloc, mobile mimalloc and bare-metal LLFF.
- [x] Isolate new native dependency opt-ins in `backends/`. Verify `cargo metadata --locked --all-features --format-version 1` and the original default selection contract.
- [ ] Run the installed-allocator alignment, realloc, zeroing and Linux memory-lock regressions. Compile board consumers for ARM, RISC-V 32/64, Xtensa, MSP430 and AVR.

## 2. Independent upstream comparison

- [x] Build the isolated three-workload survey in `examples/allocator_bench.rs` and `bench/run.py`.
- [x] In `bench/upstream.py`, pin the upstream revision, gate the untimed diagnostic and global allocator binding, and preserve all timed benchmark bodies. Share a dependency lock, release profile and Rust target within each job.
- [ ] Run all 21 Criterion cases over three rotated rounds with the baseline and every supported candidate. Save raw data, actual backend name, toolchain, commit and lockfile.
- [ ] Compare one fixed backend per target on the same set of cases, reporting a geometric mean of paired speedups and the individual regressions. Use an opt-in performance policy only where target evidence supports it. Keep physical-device coverage explicit.

## 3. ECC audit and handoff

- [ ] Install the native Codex ECC plugin with mise-managed npx. Verify the installed source and distinguish agent-configuration checks from Rust checks.
- [ ] Review selection, feature unification, firmware initialization, FFI ownership, license/advisory policy, benchmark validity and repository CI. Record audit evidence and any unverified scope.
- [ ] Save the comparison results in the repository and locally, verify their case counts, remove the temporary CI ref, and verify the feature checkout. Write the required dated NAS update.

The benchmark acceptance criterion is measured superiority on each covered target. An individual case or platform below parity remains a reported gap.
