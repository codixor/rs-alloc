"""Build each backend once, rotate execution order on one runner and save raw samples."""
import argparse
from contextlib import contextmanager
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import tempfile
from pathlib import Path


def output(*args):
    return subprocess.check_output(args, text=True).strip()


@contextmanager
def device_directory(mode):
    if mode != "android-emulator":
        yield None
        return
    path = output("adb", "shell", "mktemp", "-d", "/data/local/tmp/rs-alloc.XXXXXXXX")
    if not re.fullmatch(r"/data/local/tmp/rs-alloc\.[A-Za-z0-9]{8}", path):
        raise RuntimeError(f"Unexpected Android temporary directory: {path!r}")
    try:
        yield path
    finally:
        output("adb", "shell", "rm", "-rf", path)


def expected_backend(feature, target):
    if feature == "auto":
        if target in {"x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"}:
            return "tcmalloc"
        if target in {"x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc",
                      "x86_64-apple-darwin", "aarch64-apple-darwin",
                      "aarch64-apple-ios-sim", "x86_64-linux-android"}:
            return "mimalloc"
        if target.startswith(("x86_64-", "aarch64-")) and "-windows-" in target:
            return "mimalloc"
        return "system"
    return {"macos-jemalloc": "jemalloc", "linux-mimalloc": "mimalloc"}.get(feature, feature)


def build(target, feature, dest):
    cmd = ["cargo", "build", "--release", "--locked", "--example", "allocator_bench",
           "--no-default-features", "--target", target, "--message-format=json-render-diagnostics"]
    if feature != "system":
        cmd.extend(["--features", feature])
    artifacts = [json.loads(line) for line in output(*cmd).splitlines()]
    binary = next(a["executable"] for a in artifacts if a.get("executable")
                  and a["target"]["name"] == "allocator_bench")
    return Path(shutil.copy2(binary, dest / (feature + Path(binary).suffix)))


def cpu_model():
    if platform.system() == "Linux":
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith(("model name", "Hardware")):
                return line.split(":", 1)[1].strip()
        return output("lscpu")
    if platform.system() == "Darwin":
        return output("sysctl", "-n", "machdep.cpu.brand_string")
    return os.environ.get("PROCESSOR_IDENTIFIER", platform.processor())


def run():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--features", nargs="+", required=True)
    parser.add_argument("--mode", choices=["native", "ios-simulator", "android-emulator"], default="native")
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error("--rounds must be positive")
    rows = []
    device = platform.platform()
    with tempfile.TemporaryDirectory() as tmp, device_directory(args.mode) as device_dir:
        commands = []
        for feature in args.features:
            binary = build(args.target, feature, Path(tmp))
            if args.mode == "ios-simulator":
                udid = os.environ["SIMULATOR_UDID"]
                device = output("xcrun", "simctl", "getenv", udid, "SIMULATOR_MODEL_IDENTIFIER")
                commands.append(["xcrun", "simctl", "spawn", udid, str(binary)])
            elif args.mode == "android-emulator":
                remote = f"{device_dir}/{feature}"
                device = output("adb", "shell", "getprop", "ro.build.fingerprint")
                output("adb", "push", str(binary), remote)
                output("adb", "shell", "chmod", "700", remote)
                commands.append(["adb", "shell", remote])
            else:
                commands.append([str(binary)])
        for iteration in range(args.rounds):
            for step in range(len(commands)):
                i = (iteration + step) % len(commands)
                expected = expected_backend(args.features[i], args.target)
                for workload in ("single", "parallel", "handoff"):
                    row = json.loads(output(*commands[i], workload))
                    if row["allocator"] != expected or row["workload"] != workload:
                        raise RuntimeError(f"Unexpected measurement: {row}")
                    if row["arch"] != args.target.split("-")[0]:
                        raise RuntimeError(f"Unexpected architecture: {row}")
                    rows.append(dict(round=iteration + 1, **row))
    report = dict(target=args.target, mode=args.mode, host_cpu=cpu_model(),
                  device=device, rustc=output("rustc", "-vV"), commit=output("git", "rev-parse", "HEAD"),
                  samples=rows)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    for backend in sorted({r["allocator"] for r in rows}):
        for workload in ("single", "parallel", "handoff"):
            subset = [r["ops_per_s"] for r in rows if r["allocator"] == backend and r["workload"] == workload]
            print(f"{args.target} {args.mode} {backend} {workload}: {statistics.median(subset):,.0f} ops/s")


if __name__ == "__main__":
    run()
