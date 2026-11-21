"""Verify and summarize complete native Criterion reruns for one desktop target."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
from statistics import geometric_mean, median
from zipfile import ZipFile


def compare(path, candidate="auto", reference="upstream"):
    report = json.loads(path.read_text(encoding="utf-8"))
    if sha256(path.with_suffix(".lock").read_bytes()).hexdigest() != report["lock_sha256"]:
        raise ValueError(f"Dependency lock mismatch: {path}")
    pairs = []
    with ZipFile(path.with_suffix(".samples.zip")) as archive:
        cases = set(report["rounds"][0][reference]["median_ns"])
        if len(cases) != 21:
            raise ValueError("Expected all 21 upstream benchmark cases")
        for number, run in enumerate(report["rounds"], 1):
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
                            or min(sample["times"]) <= 0 or estimate["median"]["point_estimate"] != expected):
                        raise ValueError(f"Invalid Criterion data: {root}")
            pairs.append({case: run[reference]["median_ns"][case] / run[candidate]["median_ns"][case]
                          for case in cases})
    typical = {case: median(pair[case] for pair in pairs) for case in cases}
    return ([geometric_mean(pair.values()) for pair in pairs], geometric_mean(typical.values()), typical)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", nargs="+", type=Path, help="comparison.json from bench/upstream.py")
    parser.add_argument("--candidate", default="auto")
    parser.add_argument("--reference", default="upstream")
    args = parser.parse_args()
    for path in args.reports:
        rounds, score, ratios = compare(path, args.candidate, args.reference)
        print(path.parent.name, "rounds", *(f"{r:.3f}" for r in rounds), f"aggregate {score:.3f}")
        print("  cases above 1.05x:", sum(r > 1.05 for r in ratios.values()),
              "below 1/1.05x:", sum(r < 1 / 1.05 for r in ratios.values()))
