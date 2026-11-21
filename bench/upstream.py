"""Run the pinned upstream Criterion suite with each allocator in its own binary."""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

from run import cpu_model, output

REV = "28aea006da844756a19c91746dd994d7d765a3de"
ROOT = Path(__file__).resolve().parents[1]


def prepare(work, mobile):
    upstream = work / "upstream"
    subprocess.run(["git", "init", "-q", str(upstream)], check=True)
    subprocess.run(["git", "-C", str(upstream), "fetch", "-q", "--depth=1",
                    "https://github.com/YeautyYE/auto-allocator.git", REV], check=True)
    subprocess.run(["git", "-C", str(upstream), "checkout", "-q", "FETCH_HEAD"], check=True)
    assert output("git", "-C", str(upstream), "rev-parse", "HEAD") == REV
    original = (upstream / "benches/allocator_benchmark.rs").read_text(encoding="utf-8")
    # Gate only allocator installation and the untimed startup diagnostic.
    source = original.replace("use auto_allocator;", '#[cfg(feature = "upstream")]\nuse auto_allocator;\n'
                              '#[cfg(feature = "ours")]\nrs_alloc::install!();')
    start = source.index("    let info = auto_allocator::get_allocator_info();")
    end = source.index('    let mut group = c.benchmark_group("basic_allocation");', start)
    source = (source[:start] + '#[cfg(feature = "upstream")] {\n' + source[start:end] +
              '}\n#[cfg(feature = "ours")] println!("Benchmarking with allocator: {}", rs_alloc::NAME);\n' +
              source[end:])
    (work / "src").mkdir()
    (work / "src/main.rs").write_text(source, encoding="utf-8")
    # Upstream's mobile release defaults import an unavailable desktop-only
    # mimalloc dependency. Without those features its mobile policy is System.
    defaults = "false" if mobile else "true"
    manifest = f'''[package]
name = "allocator-comparison"
version = "0.0.0"
edition = "2021"
[dependencies]
criterion = "=0.5.1"
auto-allocator = {{ path = "upstream", optional = true, default-features = {defaults} }}
rs-alloc = {{ path = {json.dumps(ROOT.as_posix())}, optional = true, default-features = false }}
[features]
upstream = ["dep:auto-allocator"]
ours = ["dep:rs-alloc"]
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = "symbols"
'''
    (work / "Cargo.toml").write_text(manifest, encoding="utf-8")


def build(work, target, backend):
    feature = "upstream" if backend == "upstream" else "ours"
    if backend not in ("upstream", "system"):
        feature += ",rs-alloc/" + backend
    args = ["cargo", "build", "--locked", "--release", "--manifest-path", str(work / "Cargo.toml"),
            "--target", target, "--features", feature, "--message-format=json-render-diagnostics"]
    artifacts = map(json.loads, output(*args).splitlines())
    exe = next(a["executable"] for a in artifacts if a.get("executable")
               and a["target"]["name"] == "allocator-comparison")
    return Path(shutil.copy2(exe, work / (backend + Path(exe).suffix)))


def measure(mode, binary, directory):
    flags = ["--bench", "--noplot", "--sample-size", "20", "--nresamples", "1000",
             "--warm-up-time", "0.15", "--measurement-time", "0.35"]
    env = os.environ.copy()
    if mode == "android-emulator":
        remote = "/data/local/tmp/rs-alloc-criterion"
        output("adb", "shell", "mkdir", "-p", remote)
        exe = remote + "/" + binary.name
        output("adb", "push", str(binary), exe)
        output("adb", "shell", "chmod", "700", exe)
        results = remote + "/" + directory.name
        cmd = ["adb", "shell", "env", "CRITERION_HOME=" + results, exe]
    elif mode == "ios-simulator":
        env["SIMCTL_CHILD_CRITERION_HOME"] = str(directory)
        cmd = ["xcrun", "simctl", "spawn", env["SIMULATOR_UDID"], str(binary)]
    else:
        env["CRITERION_HOME"] = str(directory)
        cmd = [str(binary)]
    result = subprocess.run(cmd + flags, env=env, text=True, stdout=subprocess.PIPE, timeout=300)
    result.check_returncode()
    if mode == "android-emulator":
        output("adb", "pull", results, str(directory))
    selected = next(line.split(": ", 1)[1].strip() for line in result.stdout.splitlines()
                    if line.startswith("Benchmarking with allocator: "))
    cases = {}
    for path in directory.glob("**/new/estimates.json"):
        name = json.loads(path.with_name("benchmark.json").read_text())["full_id"]
        cases[name] = json.loads(path.read_text())["median"]["point_estimate"]
    if len(cases) != 21:
        raise RuntimeError(f"Expected 21 upstream cases, found {len(cases)} in {directory}")
    return {"allocator": selected, "median_ns": cases}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--features", nargs="+", required=True)
    parser.add_argument("--mode", choices=["native", "android-emulator", "ios-simulator"], default="native")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error("--rounds must be positive")
    backends = ["upstream", *args.features]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / "target"
    scratch.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=scratch, prefix="criterion-") as tmp:
        work = Path(tmp)
        prepare(work, args.mode != "native")
        subprocess.run(["cargo", "generate-lockfile", "--manifest-path", str(work / "Cargo.toml")], check=True)
        lock = work / "Cargo.lock"
        shutil.copy2(lock, args.output.with_suffix(".lock"))
        report = dict(target=args.target, mode=args.mode, cpu=cpu_model(), upstream=REV,
                      upstream_default_features=args.mode == "native", rustc=output("rustc", "-vV"),
                      commit=output("git", "rev-parse", "HEAD"),
                      lock_sha256=hashlib.sha256(lock.read_bytes()).hexdigest(), rounds=[])
        binaries = {name: build(work, args.target, name) for name in backends}
        for index in range(args.rounds):
            measurements = {}
            for name in backends[index % len(backends):] + backends[:index % len(backends)]:
                print(f"Round {index+1}: {name}", flush=True)
                measurement = measure(args.mode, binaries[name], work / f"round-{index+1}-{name}")
                expected = {"macos-jemalloc": "jemalloc", "linux-mimalloc": "mimalloc"}.get(name, name)
                if name != "upstream" and measurement["allocator"] != expected:
                    raise RuntimeError(f"Requested {expected}, measured {measurement['allocator']}")
                measurements[name] = measurement
            report["rounds"].append(measurements)
            args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
