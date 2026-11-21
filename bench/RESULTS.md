# Allocator benchmarks, 21 November 2026

The opt-in `auto` policy beat upstream by more than 5% on both Linux runners and both mobile simulation targets. The two Windows results and Intel macOS were within 5% of upstream. On Apple Silicon macOS, the selected backend was slower in the held-out round. Every target had individual cases within 5% or below upstream.

| Environment | Processor reported by runner | Selected backend | Held-out speedup | Wins / within 5% / losses |
|---|---|---|---:|---:|
| Linux x86_64 GNU | AMD EPYC 9V74 | TCMalloc | 1.093× | 11 / 5 / 5 |
| Linux ARM64 GNU | Arm Neoverse-N2 | TCMalloc | 1.065× | 11 / 2 / 8 |
| Windows x86_64 | AMD Family 26 Model 2 | mimalloc | 0.998× | 5 / 12 / 4 |
| Windows ARM64 | Arm Family 8 Model D49 | mimalloc | 1.026× | 6 / 13 / 2 |
| macOS Intel | Intel Core i7-8700B | mimalloc | 1.000× | 8 / 12 / 1 |
| macOS Apple Silicon | Apple M1 virtual machine | mimalloc | 0.912× | 5 / 1 / 15 |
| Android x86_64 emulator | AMD EPYC 7763 host | mimalloc | 1.888× | 15 / 6 / 0 |
| iOS ARM64 simulator | Apple M1 host | mimalloc | 1.556× | 16 / 2 / 3 |

## What was measured

Each machine ran the 21 Criterion cases in upstream revision `28aea006` with one executable per allocator. The adapter changed allocator installation and startup diagnostics. Both executables used the same timed code, compiler, target, dependency resolution and release settings. The mobile candidate labeled `mimalloc` in the archived run is exposed as `mobile-mimalloc` in the final API. Desktop upstream reported mimalloc. Mobile upstream reported System with its desktop-only optional features disabled. The archive retains the per-round median estimates for the campaign. Individual Criterion sample timings and confidence intervals were discarded by that harness. New runs save them beside the report in a `.samples.zip` file. Each case used 20 samples after a 150 ms warm-up and a 350 ms measurement target. A fresh process ran each suite, with backend order rotated across three rounds. [Upstream suite](https://github.com/YeautyYE/auto-allocator/blob/28aea006da844756a19c91746dd994d7d765a3de/benches/allocator_benchmark.rs)

For each case, speedup is upstream median time divided by candidate median time. The first two rounds selected one backend per target by the geometric mean across all cases. The table uses only the third round for scoring. A win exceeds 1.05×, a loss is below 1/1.05× and the middle band is descriptive. Results come from one hosted machine per target. The twelve basic allocation sizes and operations have twelve votes in the 21-case mean. The concurrency tests include creating and joining threads. The fragmentation case clears byte vectors while retaining their capacities.

For example, Linux x86_64 TCMalloc reached 1.093× overall but 0.809× on the four-thread concurrency case. On the iOS simulator, mimalloc reached 1.556× overall and 0.595× on the two-thread case. For a specific application, choose the column and case closest to its workload. [All paired case times](results/2026-11-21/criterion.csv)

The complementary survey measured single-producer churn, concurrent churn and cross-thread frees over five rounds. Linux and Android also recorded process high-water RSS. Its runners were provisioned separately from the Criterion run. Linux used Ubuntu's gperftools 2.15-3build1 and macOS built gperftools 2.17. The Rust bindings are tcmalloc 0.3.0, tikv-jemallocator 0.7.0 and mimalloc 0.1.52. [Survey samples](results/2026-11-21/workloads.csv) · [Raw JSON, compiler metadata and dependency locks](results/2026-11-21/raw.zip)

## Reproduce

```sh
python bench/upstream.py --target x86_64-unknown-linux-gnu --features system jemalloc linux-mimalloc tcmalloc --output target/comparison/linux/comparison.json
python bench/summarize.py target/comparison --csv target/criterion.csv
```

To replay the archived dependency resolution, extract `raw.zip` and point the current driver at the original candidate checkout. Use the compiler recorded in the target's comparison JSON and the matching native library versions for a toolchain-matched run. For example, from the repository root on x86_64 Linux:

```sh
python -m zipfile -e bench/results/2026-11-21/raw.zip target/replay-input
git worktree add --detach .worktrees/criterion-replay ebe2686a48400a31f8aa70379d6da55919486409
python bench/upstream.py --candidate-dir .worktrees/criterion-replay --lockfile target/replay-input/upstream/native-x86_64-unknown-linux-gnu/comparison.lock --target x86_64-unknown-linux-gnu --features system jemalloc linux-mimalloc tcmalloc --output target/replayed/linux/comparison.json
git worktree remove .worktrees/criterion-replay
```

The upstream suite requires the Rust standard library, OS threads and a clock. Firmware validation compiled a downstream heap consumer with the ARM Cortex-M, AVR, MSP430, RISC-V 32/64 and Xtensa toolchains. The run also cross-checked ARM32 and RISC-V Linux, physical iOS ARM64 and Android ARM32/ARM64. Physical handset and board timings remain unmeasured. [Hosted benchmark and firmware checks](https://github.com/codixor/rs-alloc/actions/runs/44860123848) · [Complementary survey](https://github.com/codixor/rs-alloc/actions/runs/44859305682)
