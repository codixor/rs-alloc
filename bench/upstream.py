"""Run the pinned upstream Criterion suite with each allocator in its own binary."""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path

from run import cpu_model, device_directory, expected_backend, output

REV = "28aea006da844756a19c91746dd994d7d765a3de"
ROOT = Path(__file__).resolve().parents[1]


def prepare(work, mobile, candidate, features):
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
              '}\n#[cfg(feature = "ours")] { println!("Benchmarking with allocator: {}", rs_alloc::NAME);\n' +
              'println!("Allocator type: {}", std::any::type_name::<rs_alloc::Allocator>()); }\n' +
              source[end:])
    (work / "src").mkdir()
    (work / "src/main.rs").write_text(source, encoding="utf-8")
    # Upstream's mobile release defaults import an unavailable desktop-only
    # mimalloc dependency. Without those features its mobile policy is System.
    defaults = "false" if mobile else "true"
    # Cargo resolves declared feature edges when generating the shared lock.
    # Merely passing dependency/feature on the build command line can leave
    # optional transitive dependencies out of the archived resolution.
    # Archived candidates used the package name rs-alloc. Keep a stable import
    # and feature alias when replaying their locks or measuring the release.
    candidate_manifest = (candidate / "Cargo.toml").resolve()
    metadata = json.loads(output("cargo", "metadata", "--no-deps", "--format-version", "1",
                                 "--manifest-path", str(candidate_manifest)))
    package = next(p["name"] for p in metadata["packages"]
                   if Path(p["manifest_path"]).resolve() == candidate_manifest)
    feature_edges = "\n".join(f'case-{name} = ["ours", "rs-alloc/{name}"]'
                              for name in features if name != "system")
    manifest = f'''[workspace]
exclude = ["upstream"]

[package]
name = "allocator-comparison"
version = "0.0.0"
edition = "2021"
[dependencies]
criterion = "=0.5.1"
auto-allocator = {{ path = "upstream", optional = true, default-features = {defaults} }}
rs-alloc = {{ package = {json.dumps(package)}, path = {json.dumps(candidate.as_posix())}, optional = true, default-features = false }}
[features]
upstream = ["dep:auto-allocator"]
ours = ["dep:rs-alloc"]
{feature_edges}
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = "symbols"
'''
    (work / "Cargo.toml").write_text(manifest, encoding="utf-8")


def build(work, target, backend, legacy_lock):
    feature = {"upstream": "upstream", "system": "ours"}.get(backend, "case-" + backend)
    if legacy_lock and backend not in ("upstream", "system"):
        feature = "ours,rs-alloc/" + backend
    args = ["cargo", "build", "--locked", "--release", "--manifest-path", str(work / "Cargo.toml"),
            "--target", target, "--features", feature, "--message-format=json-render-diagnostics"]
    artifacts = map(json.loads, output(*args).splitlines())
    exe = next(a["executable"] for a in artifacts if a.get("executable")
               and a["target"]["name"] == "allocator-comparison")
    return Path(shutil.copy2(exe, work / (backend + Path(exe).suffix)))


def measure(mode, binary, directory, remote, archive):
    flags = ["--bench", "--noplot", "--sample-size", "20", "--nresamples", "1000",
             "--warm-up-time", "0.15", "--measurement-time", "0.35"]
    env = os.environ.copy()
    if mode == "android-emulator":
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
        for file in ("benchmark.json", "sample.json", "estimates.json"):
            archive.write(path.with_name(file), directory.name + "/" + name + "/" + file)
    if len(cases) != 21:
        raise RuntimeError(f"Expected 21 upstream cases, found {len(cases)} in {directory}")
    adapter = next((line.removeprefix("Allocator type: ") for line in result.stdout.splitlines()
                    if line.startswith("Allocator type: ")), None)
    return {"allocator": selected, "adapter": adapter, "median_ns": cases}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--candidate-dir", type=Path, default=ROOT, help="rs-alloc checkout to benchmark")
    parser.add_argument("--lockfile", type=Path, help="replay an existing comparison.lock exactly")
    parser.add_argument("--legacy-lock", action="store_true",
                        help="replay a historical lock that omitted optional feature edges")
    parser.add_argument("--features", nargs="+", required=True)
    parser.add_argument("--mode", choices=["native", "android-emulator", "ios-simulator"], default="native")
    parser.add_argument("--balanced-order", action="store_true",
                        help="pair each rotation with its reverse to balance early and late runs")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if (len(set(args.features)) != len(args.features) or "upstream" in args.features
            or any(not re.fullmatch(r"[A-Za-z0-9_-]+", f) for f in args.features)):
        parser.error("provide distinct Cargo feature names or system; upstream is included automatically")
    if args.rounds < 1:
        parser.error("--rounds must be positive")
    if args.legacy_lock and not args.lockfile:
        parser.error("--legacy-lock requires the original --lockfile")
    legacy_lock = args.legacy_lock
    if args.lockfile and args.lockfile.with_suffix(".json").is_file():
        prior = json.loads(args.lockfile.with_suffix(".json").read_text(encoding="utf-8"))
        if "resolved_features" not in prior or prior["resolved_features"] is None:
            legacy_lock = True
    candidate = args.candidate_dir.resolve(strict=True)
    backends = ["upstream", *args.features]
    if args.balanced_order and args.rounds % (2 * len(backends)):
        parser.error("balanced order requires a whole cycle of forward and reverse rotations")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    scratch = ROOT / "target"
    scratch.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(dir=scratch, prefix="criterion-") as tmp, device_directory(args.mode) as remote:
        work = Path(tmp)
        prepare(work, args.mode != "native", candidate, [] if legacy_lock else args.features)
        lock = work / "Cargo.lock"
        if args.lockfile:
            shutil.copyfile(args.lockfile, lock)
        else:
            subprocess.run(["cargo", "generate-lockfile", "--manifest-path", str(work / "Cargo.toml")], check=True)
        if args.lockfile is None or args.lockfile.resolve() != args.output.with_suffix(".lock").resolve():
            shutil.copy2(lock, args.output.with_suffix(".lock"))
        report = dict(target=args.target, mode=args.mode, cpu=cpu_model(), upstream=REV,
                      upstream_default_features=args.mode == "native", rustc=output("rustc", "-vV"),
                      commit=output("git", "-C", str(candidate), "rev-parse", "HEAD"),
                      requested_rounds=args.rounds, balanced_order=args.balanced_order,
                      resolved_features=None if legacy_lock else args.features,
                      lock_sha256=hashlib.sha256(lock.read_bytes()).hexdigest(), rounds=[])
        binaries = {name: build(work, args.target, name, legacy_lock) for name in backends}
        if hashlib.sha256(lock.read_bytes()).hexdigest() != report["lock_sha256"]:
            raise RuntimeError("Dependency resolution changed while building the comparison")
        with zipfile.ZipFile(args.output.with_suffix(".samples.zip"), "w", zipfile.ZIP_DEFLATED) as archive:
            for index in range(args.rounds):
                measurements = {}
                offset = (index // 2 if args.balanced_order else index) % len(backends)
                order = backends[offset:] + backends[:offset]
                if args.balanced_order and index % 2:
                    order.reverse()
                for name in order:
                    print(f"Round {index+1}: {name}", flush=True)
                    measurement = measure(args.mode, binaries[name], work / f"round-{index+1}-{name}", remote, archive)
                    expected = expected_backend(name, args.target)
                    if name != "upstream" and measurement["allocator"] != expected:
                        raise RuntimeError(f"Requested {expected}, measured {measurement['allocator']}")
                    measurements[name] = measurement
                report["rounds"].append(measurements)
                args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
