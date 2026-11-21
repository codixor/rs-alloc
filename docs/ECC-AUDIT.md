# rs-alloc ECC audit, 21 November 2026

The ECC Rust and security review found no confirmed vulnerability in the allocator selector or installation macros at `866c19dddeb88aaa804c2250a7b65040031e874e`. RustSec scanned 22 locked packages. Cargo-deny passed its advisories, bans, licenses and sources checks. It reported one unused `Unicode-3.0` license allowance.

## Scope and findings

The review traced Cargo feature combinations through native dependency selection, the installed `GlobalAlloc` type, board-owned heap initialization, and benchmark subprocesses. It used the ECC Rust reviewer, security review and verification workflow. The separate ECC harness inventory evaluated agent configuration. [ECC Rust review](https://github.com/affaan-m/ECC/blob/4adeee6732a12d1540f49a4d50a38ce8c97f9a53/commands/rust-review.md)

| Finding | Resolution and verification |
|---|---|
| Existing desktop `mimalloc` feature changed mobile behavior | A separate `mobile-mimalloc` opt-in preserves the previous mobile System policy. Dependency graphs and physical-target compile checks verified both legacy and automatic fallbacks. |
| Unrelated features introduced a native C build | Checked macOS jemalloc in isolation, Linux-only mimalloc on Windows, and the physical ARM Android/iOS System fallback. Each excludes the native mimalloc library. |
| Linux auto policy included 32-bit-pointer GNU ABIs | The TCMalloc rule now requires 64-bit pointers. Downstream compile-time assertions verified System on GNU x32 and AArch64 ILP32 with default features off. |
| TCMalloc resize and zeroed-allocation documentation overstated guarantees | The README describes the Rust adapter's inherited `GlobalAlloc` methods. Linux tests exercised the linked gperftools library. |
| Android benchmark runs shared files on the same device | Each invocation creates and validates a private scratch directory. The cleanup check covered simultaneous sessions and a benchmark failure. |
| Archived dependency versions could drift on replay | The driver accepts a pinned candidate checkout and lockfile. A Windows replay used an identical lock hash and wrote all 42 Criterion sample sets for the two binaries. |

## Verification and limits

- Formatting, Clippy with warnings denied, Linux release build and rustdoc passed. Rust 1.85 tests exercised default Linux jemalloc and explicit automatic TCMalloc. Windows tests verified backend selection, alignment, growth and zeroing.
- An independent review recomputed all 1,512 paired Criterion rows, all 360 complementary survey observations and the eight held-out results. The original campaign retained case medians. Future runs also archive individual Criterion sample timings and confidence intervals. [Benchmark report](../bench/RESULTS.md)
- The temporary comparison ran on eight hosted environments and built a downstream firmware library on six bare-metal target families. Physical handset timings, linked board firmware, the native allocator implementations themselves, and system-library security updates remain outside the verified scope. The permanent CI retains its previous platform coverage. [Completed comparison run](https://github.com/codixor/rs-alloc/actions/runs/44860123848)

ECC 2.2.3 was installed for native Codex through mise-managed npx. The audit used the official marketplace revision `4adeee6732a12d1540f49a4d50a38ce8c97f9a53`. The RustSec database revision was `f291af9b1cc0d21dfcea1709d6b037963cef262b`.
