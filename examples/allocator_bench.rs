//! Each invocation measures one workload with one installed backend.
use std::hint::black_box;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

rs_alloc::install!();

const BATCH: usize = 128;
const BATCHES: usize = 1024;

fn block(sequence: usize) -> Vec<u8> {
    // The same mix of 32 B through 32 KiB on every target. Write every byte.
    let sizes = [32, 64, 96, 256, 512, 1024, 4096, 32768];
    black_box(vec![sequence as u8; sizes[sequence % sizes.len()]])
}

fn churn(worker: usize) -> Vec<u64> {
    let mut live = vec![Vec::new(); 97];
    let mut samples = Vec::with_capacity(BATCHES);
    for batch in 0..BATCHES {
        let start = Instant::now();
        for index in 0..BATCH {
            let seq = batch * BATCH + index + worker;
            let slot = seq % live.len();
            live[slot] = block(seq);
        }
        samples.push(start.elapsed().as_nanos() as u64);
    }
    black_box(live);
    samples
}

fn handoff() -> Vec<u64> {
    let (send, recv) = mpsc::sync_channel::<Vec<Vec<u8>>>(2);
    thread::scope(|scope| {
        scope.spawn(move || {
            for batch in recv {
                drop(black_box(batch));
            }
        });
        let mut samples = Vec::with_capacity(BATCHES);
        for batch in 0..BATCHES {
            let start = Instant::now();
            send.send((0..BATCH).map(|i| block(batch * BATCH + i)).collect())
                .unwrap();
            samples.push(start.elapsed().as_nanos() as u64);
        }
        drop(send);
        samples
    })
}

fn main() {
    let workload = std::env::args()
        .nth(1)
        .expect("single, parallel or handoff");
    let workers = match workload.as_str() {
        "single" | "handoff" => 1,
        "parallel" => thread::available_parallelism().unwrap().get().min(4),
        _ => panic!("unknown workload"),
    };
    // Warm the backend before timing. Each process handles just one workload.
    black_box((0..BATCH).map(block).collect::<Vec<_>>());
    let start = Instant::now();
    let mut samples = match workload.as_str() {
        "single" => churn(0),
        "handoff" => handoff(),
        _ => thread::scope(|scope| {
            let jobs: Vec<_> = (0..workers)
                .map(|id| scope.spawn(move || churn(id)))
                .collect();
            jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
        }),
    };
    let elapsed = start.elapsed().as_secs_f64();
    let ops_per_sec = (BATCH * samples.len()) as f64 / elapsed;
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99).div_ceil(100) - 1];
    let peak = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find_map(|line| line.strip_prefix("VmHWM:"))?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        });
    println!(
        r#"{{"allocator":"{}","arch":"{}","os":"{}","workload":"{}","producers":{},"ops_per_s":{:.0},"batch_p99_ns":{},"peak_rss_kib":{}}}"#,
        rs_alloc::NAME,
        std::env::consts::ARCH,
        std::env::consts::OS,
        workload,
        workers,
        ops_per_sec,
        p99,
        peak.map_or_else(|| "null".into(), |kb| kb.to_string())
    );
}
