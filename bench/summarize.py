"""Summarize a three-round upstream comparison and export every paired estimate."""
import argparse
import csv
import json
from pathlib import Path
from statistics import geometric_mean, median

FIELDS = ["target", "mode", "feature", "allocator", "case", "round", "upstream_ns", "rs_alloc_ns"]


def summarize(report):
    rounds = report["rounds"]
    if len(rounds) != 3:
        raise ValueError("Expected two selection rounds and one held-out round")
    cases = sorted(rounds[0]["upstream"]["median_ns"])
    features = sorted(set(rounds[0]) - {"upstream"})
    if len(cases) != 21 or not features:
        raise ValueError("Incomplete Criterion suite")
    records, scores = [], {}
    for feature in features:
        ratios = []
        for i, sample in enumerate(rounds, start=1):
            baseline = sample["upstream"]["median_ns"]
            candidate = sample[feature]["median_ns"]
            if set(baseline) != set(cases) or set(candidate) != set(cases):
                raise ValueError(f"Mismatched case set in round {i} for {feature}")
            ratios.append({case: baseline[case] / candidate[case] for case in cases})
            for case in cases:
                records.append([report["target"], report["mode"], feature, sample[feature]["allocator"],
                                case, i, baseline[case], candidate[case]])
        training = geometric_mean(median(r[case] for r in ratios[:2]) for case in cases)
        holdout = ratios[2]
        worst = min(holdout, key=holdout.get)
        scores[feature] = (training, geometric_mean(holdout.values()),
                           sum(v > 1.05 for v in holdout.values()),
                           sum(v < 1 / 1.05 for v in holdout.values()), worst, holdout[worst])
    return records, scores


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", type=Path, help="directory of downloaded comparison.json artifacts")
    parser.add_argument("--csv", type=Path, required=True)
    args = parser.parse_args()
    paths = sorted(args.reports.rglob("comparison.json"))
    if not paths:
        parser.error("No comparison.json reports found")
    args.csv.parent.mkdir(parents=True, exist_ok=True)
    with args.csv.open("w", newline="", encoding="utf-8") as stream:
        csv_rows = csv.writer(stream)
        csv_rows.writerow(FIELDS)
        for path in paths:
            report = json.loads(path.read_text(encoding="utf-8"))
            rows, scores = summarize(report)
            csv_rows.writerows(rows)
            best = max(scores, key=lambda feature: scores[feature][0])
            print(report["target"], report["mode"], "selected", best)
            for name, (train, test, wins, losses, worst, ratio) in scores.items():
                print(f"  {name}: selection {train:.3f}x; holdout {test:.3f}x; "
                      f"{wins} wins, {21-wins-losses} within 5%, {losses} losses; "
                      f"slowest {worst} {ratio:.3f}x")


if __name__ == "__main__":
    main()
