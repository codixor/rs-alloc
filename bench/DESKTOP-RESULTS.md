# Desktop allocator benchmarks, 21 November 2026

The opt-in `auto` allocator beat upstream across the 21-case suite on all four desktop runners. The Apple Silicon result moved from 0.912× in the earlier survey to 1.108× in the six-round confirmation. Intel macOS and Windows ARM64 also exceeded the survey's 5% comparison band. The 1.036× Windows x64 result was a smaller gain. [Earlier survey](RESULTS.md) · [Fresh confirmation](https://github.com/codixor/rs-alloc/actions/runs/44866952595)

| Native runner | Earlier held-out ratio | Confirmed vs upstream | Confirmed vs stock mimalloc | Rounds faster than upstream | Case wins / within 5% / losses |
|---|---:|---:|---:|---:|---:|
| macOS, Apple M1 VM | 0.912× | 1.108× | 1.111× | 5 of 6 | 13 / 8 / 0 |
| macOS, Intel Core i7-8700B | 1.000× | 1.089× | 1.103× | 6 of 6 | 13 / 8 / 0 |
| Windows ARM64, Arm Family 8 Model D49 | 1.026× | 1.089× | 1.056× | 6 of 6 | 15 / 6 / 0 |
| Windows x64, AMD Family 26 Model 2 | 0.998× | 1.036× | 1.065× | 5 of 6 | 8 / 11 / 2 |

Each confirmed ratio is the geometric mean across 21 cases. For each case, we took the median of its six paired speedups (reference time divided by candidate time). Case wins exceed 1.05× and losses fall below 1/1.05×. The earlier column uses the original survey's single held-out round. Each new comparison ran on one fresh hosted machine per target. Apple Silicon varied most, with round aggregates from 0.872× to 1.233×. Windows x64's 12 basic cases scored 1.090× and its other nine cases scored 0.969×. Its JSON-like serialization case scored 0.828×. [All paired confirmation measurements](results/2026-11-21-desktop/confirmation.csv)

## What changed

The stock mimalloc Rust adapter uses aligned native calls for ordinary Rust allocations. The desktop automatic adapter dispatches pointer-aligned or less strictly aligned requests to the ordinary and small-block APIs when their size permits. Stricter layouts use the aligned APIs. On macOS and Windows ARM64 the adapter clears the requested bytes directly for ordinary zeroed allocations up to 64 KiB. Windows x64 performed better with native zeroing. Both paths retain native zeroing for large allocations. Explicit `mimalloc` uses the stock Rust adapter. [Adapter source](https://github.com/codixor/rs-alloc/blob/041e03a51298892e17c8313814487d2940dffbe0/desktop/src/lib.rs) · [mimalloc Rust bindings](https://github.com/purpleprotocol/mimalloc_rust)

## How the policy was selected and confirmed

1. The pilot ran Apple Silicon first. The native small-block path reached 1.109× upstream in the third round, though the stock adapter was faster on that machine. Windows and Intel macOS then ran the same pilot. Its historical lock captures the harness base dependencies. [Pilot run](https://github.com/codixor/rs-alloc/actions/runs/44865018773) · [Pilot reports and samples](results/2026-11-21-desktop/pilot.zip)
2. Four-way tuning rotated upstream, stock mimalloc, native zeroing and Rust-sized zeroing through every execution position. Each target's four-round aggregate selected the production path. The comparison manifest declares every candidate feature dependency before generating the shared lock. [Tuning run](https://github.com/codixor/rs-alloc/actions/runs/44865787602) · [Tuning reports and samples](results/2026-11-21-desktop/tuning.zip)
3. Confirmation used frozen allocator commit `08b865e`, upstream revision `28aea006`, Rust 1.100.0 and the tuning lock with SHA-256 `c0480970322b69f9d033ac358fa402fb50a68ef9dd2a6048c402c31d2bebd0b5`. Each machine ran upstream, stock mimalloc and production `auto` in all six permutations. Each binary used the same 21 timed bodies, 20 samples per case, 150 ms warm-up and 350 ms measurement target. The archive includes the concrete Rust adapter names and every Criterion sample, case descriptor and estimate. An initial confirmation attempt stopped during Cargo workspace discovery before timing any cases. [Upstream suite](https://github.com/YeautyYE/auto-allocator/blob/28aea006da844756a19c91746dd994d7d765a3de/benches/allocator_benchmark.rs) · [Confirmation reports and samples](results/2026-11-21-desktop/confirmation.zip)

## Windows x64 follow-up

A separate AMD Windows runner compared the production native-reallocation path with an experimental small-block copy on growth. In eight mirrored rounds, production `auto` scored 1.051× upstream and was faster in all eight rounds. The copy experiment scored 1.061× upstream and 1.008× against production. Its 21 case ratios against production ranged from 0.987× to 1.033×. The shipping policy keeps native reallocation, which can grow a block in place. Across the two independent Windows x64 machines, that policy measured 1.036× and 1.051× upstream. [Eight-round Windows comparison](https://github.com/codixor/rs-alloc/actions/runs/44867773976) · [Raw Windows reports and samples](results/2026-11-21-desktop/windows-x64-growth.zip)

## Reproduce or verify

For example, on an x64 Windows host with Rust 1.100.0, run the following from the repository. Use the corresponding target triple and lock on the other native runners.

```sh
python -m zipfile -e bench/results/2026-11-21-desktop/confirmation.zip target/desktop-data
python bench/confirm.py target/desktop-data/x86_64-pc-windows-msvc/comparison.json --rounds 6 --require-balanced --adapter rs_alloc_desktop::DesktopMiMalloc
git worktree add --detach ../rs-alloc-confirmed 08b865e
rustup run 1.100.0 python bench/upstream.py --candidate-dir ../rs-alloc-confirmed --target x86_64-pc-windows-msvc --features mimalloc auto --rounds 6 --balanced-order --lockfile target/desktop-data/x86_64-pc-windows-msvc/comparison.lock --output target/desktop-replay/comparison.json
```
