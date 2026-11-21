"""Verify and summarize complete native Criterion reruns for one desktop target."""
import argparse
from hashlib import sha256
import json
from math import isclose, isfinite
from pathlib import Path
from statistics import geometric_mean, median
from zipfile import ZipFile


def compare(path, candidate="auto", reference="upstream", *, expected_rounds=None,
            require_balanced=False, adapter=None):
    report = json.loads(path.read_text(encoding="utf-8"))
    if sha256(path.with_suffix(".lock").read_bytes()).hexdigest() != report["lock_sha256"]:
        raise ValueError(f"Dependency lock mismatch: {path}")
    declared = report.get("requested_rounds")
    count = expected_rounds if expected_rounds is not None else declared
    if not count or (declared is not None and declared != count) or len(report["rounds"]) != count:
        raise ValueError(f"Incomplete run or unknown round count in {path}; historical runs require --rounds")
    balanced = report.get("balanced_order", False)
    names = list(report["rounds"][0])
    if not names or names[0] != "upstream" or reference == candidate:
        raise ValueError("Invalid reference or candidate configuration")
    if require_balanced and not balanced or balanced and count % (2 * len(names)):
        raise ValueError("A complete forward/reverse rotation is required")
    pairs = []
    with ZipFile(path.with_suffix(".samples.zip")) as archive:
        cases = set(report["rounds"][0][reference]["median_ns"])
        if len(cases) != 21:
            raise ValueError("Expected all 21 upstream benchmark cases")
        for number, run in enumerate(report["rounds"], 1):
            offset = ((number - 1) // 2 if balanced else number - 1) % len(names)
            order = names[offset:] + names[:offset]
            if balanced and number % 2 == 0:
                order.reverse()
            if list(run) != order:
                raise ValueError(f"Unexpected backend order in round {number}")
            if adapter is not None and run[candidate].get("adapter") != adapter:
                raise ValueError(f"Unexpected concrete allocator for {candidate} in round {number}")
            for backend in (reference, candidate):
                estimates = run[backend]["median_ns"]
                if set(estimates) != cases:
                    raise ValueError(f"Different cases for {backend} in round {number}")
                for case, expected in estimates.items():
                    root = f"round-{number}-{backend}/{case}"
                    meta = json.loads(archive.read(root + "/benchmark.json"))
                    sample = json.loads(archive.read(root + "/sample.json"))
                    estimate = json.loads(archive.read(root + "/estimates.json"))
                    if (meta["full_id"] != case or len(sample["times"]) != 20
                            or len(sample["iters"]) != 20 or min(sample["iters"]) <= 0
                            or min(sample["times"]) <= 0 or not isfinite(expected) or expected <= 0
                            or estimate["median"]["point_estimate"] != expected):
                        raise ValueError(f"Invalid Criterion data: {root}")
                    raw_median = median(time / iterations for time, iterations
                                        in zip(sample["times"], sample["iters"]))
                    if not isclose(raw_median, expected, rel_tol=1e-12):
                        raise ValueError(f"Raw sample median disagrees with reported estimate: {root}")
            pairs.append({case: run[reference]["median_ns"][case] / run[candidate]["median_ns"][case]
                          for case in cases})
    typical = {case: median(pair[case] for pair in pairs) for case in cases}
    return ([geometric_mean(pair.values()) for pair in pairs], geometric_mean(typical.values()), typical)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", nargs="+", type=Path, help="comparison.json from bench/upstream.py")
    parser.add_argument("--candidate", default="auto")
    parser.add_argument("--reference", default="upstream")
    parser.add_argument("--rounds", type=int, help="expected rounds, required for historical reports")
    parser.add_argument("--require-balanced", action="store_true", help="require complete mirrored rotations")
    parser.add_argument("--adapter", help="expected concrete Rust allocator type for the candidate")
    args = parser.parse_args()
    for path in args.reports:
        rounds, score, ratios = compare(path, args.candidate, args.reference,
                                       expected_rounds=args.rounds, require_balanced=args.require_balanced,
                                       adapter=args.adapter)
        print(path.parent.name, "rounds", *(f"{r:.3f}" for r in rounds), f"aggregate {score:.3f}")
        print("  cases above 1.05x:", sum(r > 1.05 for r in ratios.values()),
              "below 1/1.05x:", sum(r < 1 / 1.05 for r in ratios.values()))
